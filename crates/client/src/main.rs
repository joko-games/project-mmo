// A release build opens no console window beside the game on Windows.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod capture;
mod hud;
mod menu;
mod net;
mod probe;
mod sky;
mod system;

use bevy::prelude::*;
use mmo_protocol::Welcome;

/// Where the game looks for its galaxy unless `--server host:port` says otherwise.
const DEFAULT_SERVER: &str = "127.0.0.1:7777";

#[derive(States, Default, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Screen {
    #[default]
    Menu,
    Playing,
}

/// The server address the game connects to (`--server host:port`). `--connect` presses WAKE UP
/// at once, for testing.
#[derive(Resource)]
pub struct ServerAddress(pub String);

/// What the server said when the player came in, and when (by the game's clock).
#[derive(Resource)]
pub struct Session {
    pub name: String,
    pub welcome: Welcome,
    pub started: f64,
}

/// Everything that belongs to a session, removed when it ends.
#[derive(Component)]
pub struct InGame;

fn main() {
    let mut server = DEFAULT_SERVER.to_string();
    let mut auto_connect = false;
    let mut capture = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        if arg == "--server"
            && let Some(value) = args.next()
        {
            server = value;
        }
        if arg == "--capture"
            && let Some(value) = args.next()
        {
            capture = Some(value);
        }
        if arg == "--connect" {
            auto_connect = true;
        }
    }

    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Project MMO v{}", env!("CARGO_PKG_VERSION")),
                ..default()
            }),
            ..default()
        }))
        .insert_resource(ClearColor(Color::BLACK))
        .insert_resource(ServerAddress(server))
        .insert_resource(menu::AutoConnect(auto_connect || capture.is_some()))
        .insert_resource(capture::Capture(capture))
        .add_plugins(capture::CapturePlugin)
        .init_state::<Screen>()
        .add_plugins((
            sky::SkyPlugin,
            menu::MenuPlugin,
            system::SystemPlugin,
            probe::ProbePlugin,
            hud::HudPlugin,
        ))
        .add_systems(OnExit(Screen::Playing), end_session)
        .run();
}

fn end_session(mut commands: Commands, entities: Query<Entity, With<InGame>>) {
    for entity in &entities {
        commands.entity(entity).despawn();
    }
    commands.remove_resource::<Session>();
}
