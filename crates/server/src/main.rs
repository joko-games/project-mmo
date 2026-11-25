//! A galaxy server: one running instance is one galaxy.
//!
//! It accepts probes over TCP, gives each new one a home system of its own, and remembers where
//! every probe is between sessions in `data/<galaxy id>.json`.
//!
//!     mmo-server [--port 7777] [--galaxy-id milky-0] [--seed 1234] [--data data]

mod galaxy;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use mmo_protocol::{
    ClientMessage, DEFAULT_PORT, PROTOCOL_VERSION, ServerMessage, decode, encode, is_valid_name,
};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

use galaxy::Galaxy;

struct Options {
    port: u16,
    galaxy_id: String,
    seed: u64,
    data: PathBuf,
}

fn options() -> Result<Options> {
    let mut options = Options {
        port: DEFAULT_PORT,
        galaxy_id: "galaxy-0".into(),
        seed: 0x5EED_0001,
        data: PathBuf::from("data"),
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let mut value = || args.next().with_context(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--port" => options.port = value()?.parse()?,
            "--galaxy-id" => options.galaxy_id = value()?,
            "--seed" => options.seed = value()?.parse()?,
            "--data" => options.data = value()?.into(),
            other => anyhow::bail!("unknown option {other}"),
        }
    }
    Ok(options)
}

#[tokio::main]
async fn main() -> Result<()> {
    let options = options()?;
    let galaxy = Galaxy::load_or_create(&options.data, &options.galaxy_id, options.seed)?;
    println!(
        "galaxy {} ({}), {} probe(s) known",
        galaxy.info.name,
        galaxy.info.id,
        galaxy.probe_count()
    );
    let galaxy = Arc::new(Mutex::new(galaxy));

    // Saved every few seconds when something changed, and once more on Ctrl+C.
    let saver = galaxy.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(5));
        loop {
            tick.tick().await;
            if let Err(error) = saver.lock().await.save_if_dirty() {
                eprintln!("saving the galaxy failed: {error:#}");
            }
        }
    });

    let listener = TcpListener::bind(("0.0.0.0", options.port))
        .await
        .with_context(|| format!("listening on port {}", options.port))?;
    println!("listening on port {}", options.port);

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, address) = accepted?;
                let galaxy = galaxy.clone();
                tokio::spawn(async move {
                    if let Err(error) = serve(stream, address, galaxy).await {
                        println!("{address}: {error:#}");
                    }
                });
            }
            _ = tokio::signal::ctrl_c() => {
                galaxy.lock().await.save()?;
                println!("saved, stopping");
                return Ok(());
            }
        }
    }
}

async fn serve(stream: TcpStream, address: SocketAddr, galaxy: Arc<Mutex<Galaxy>>) -> Result<()> {
    stream.set_nodelay(true)?;
    let (read, mut write) = stream.into_split();
    let mut lines = BufReader::new(read).lines();

    let hello = tokio::time::timeout(Duration::from_secs(10), lines.next_line())
        .await
        .context("no hello in time")??
        .context("closed before hello")?;
    let name = match decode::<ClientMessage>(&hello)? {
        ClientMessage::Hello { protocol, name } => {
            let refusal = if protocol != PROTOCOL_VERSION {
                Some(format!(
                    "this galaxy speaks protocol {PROTOCOL_VERSION}, the game speaks {protocol}: update the game"
                ))
            } else if !is_valid_name(&name) {
                Some("names are 3 to 24 letters, digits, - or _".to_string())
            } else {
                None
            };
            if let Some(reason) = refusal {
                write
                    .write_all(encode(&ServerMessage::Rejected { reason }).as_bytes())
                    .await?;
                return Ok(());
            }
            name
        }
        other => anyhow::bail!("expected hello, got {other:?}"),
    };

    let welcome = galaxy.lock().await.arrive(&name);
    println!(
        "{address}: {name} wakes up in {} (system #{}){}",
        welcome.home.name,
        welcome.home.index,
        if welcome.first_arrival {
            ", first arrival"
        } else {
            ""
        }
    );
    write
        .write_all(encode(&ServerMessage::Welcome(welcome)).as_bytes())
        .await?;

    while let Some(line) = lines.next_line().await? {
        match decode::<ClientMessage>(&line) {
            Ok(ClientMessage::ProbeState { position, rotation }) => {
                galaxy.lock().await.moved(&name, position, rotation);
            }
            Ok(ClientMessage::IntroSeen) => galaxy.lock().await.intro_seen(&name),
            Ok(ClientMessage::Hello { .. }) => {}
            Err(error) => println!("{address}: unreadable message: {error}"),
        }
    }
    println!("{address}: {name} disconnected");
    Ok(())
}
