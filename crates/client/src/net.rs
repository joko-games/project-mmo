//! The connection to the galaxy server: a thread reads, another writes, and the game exchanges
//! messages with them through channels, never blocking a frame.

use std::io::{BufRead, BufReader, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
use std::sync::Mutex;
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use bevy::prelude::*;
use mmo_protocol::{ClientMessage, PROTOCOL_VERSION, ServerMessage, decode, encode};

pub enum NetEvent {
    Message(ServerMessage),
    /// Could not connect, or the connection broke: what to tell the player.
    Lost(String),
}

/// The live connection, while there is one.
#[derive(Resource)]
pub struct Connection {
    outgoing: Sender<ClientMessage>,
    incoming: Mutex<Receiver<NetEvent>>,
}

impl Connection {
    /// Starts connecting at once; the outcome arrives as a [`NetEvent`].
    pub fn open(address: String, name: String) -> Self {
        let (outgoing, to_send) = mpsc::channel::<ClientMessage>();
        let (events, incoming) = mpsc::channel::<NetEvent>();
        std::thread::spawn(move || {
            if let Err(error) = run(&address, &name, to_send, &events) {
                events.send(NetEvent::Lost(error)).ok();
            }
        });
        Self {
            outgoing,
            incoming: Mutex::new(incoming),
        }
    }

    pub fn send(&self, message: ClientMessage) {
        self.outgoing.send(message).ok();
    }

    /// Everything that arrived since the last call.
    pub fn drain(&self) -> Vec<NetEvent> {
        let incoming = self.incoming.lock().expect("never poisoned");
        incoming.try_iter().collect()
    }
}

fn run(
    address: &str,
    name: &str,
    to_send: Receiver<ClientMessage>,
    events: &Sender<NetEvent>,
) -> Result<(), String> {
    let target: SocketAddr = address
        .to_socket_addrs()
        .map_err(|e| format!("bad server address {address}: {e}"))?
        .next()
        .ok_or_else(|| format!("bad server address {address}"))?;
    let stream = TcpStream::connect_timeout(&target, Duration::from_secs(3))
        .map_err(|e| format!("no galaxy answers at {address} ({e}). Is the server running?"))?;
    stream.set_nodelay(true).ok();

    let mut writer = stream.try_clone().map_err(|e| e.to_string())?;
    let hello = encode(&ClientMessage::Hello {
        protocol: PROTOCOL_VERSION,
        name: name.to_string(),
    });
    writer
        .write_all(hello.as_bytes())
        .map_err(|e| e.to_string())?;
    std::thread::spawn(move || {
        for message in to_send {
            if writer.write_all(encode(&message).as_bytes()).is_err() {
                break;
            }
        }
    });

    for line in BufReader::new(stream).lines() {
        let line = line.map_err(|e| format!("connection lost: {e}"))?;
        match decode::<ServerMessage>(&line) {
            Ok(message) => {
                if events.send(NetEvent::Message(message)).is_err() {
                    return Ok(());
                }
            }
            Err(error) => warn!("unreadable message from the server: {error}"),
        }
    }
    Err("the server closed the connection".into())
}
