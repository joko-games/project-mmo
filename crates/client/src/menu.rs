//! The first screen: the probe's name, filled in at random, and a button to wake it up.

use bevy::input_focus::{FocusCause, InputFocus};
use bevy::prelude::*;
use bevy::text::EditableText;
use mmo_protocol::procgen::Rng;
use mmo_protocol::{MAX_NAME_CHARS, ServerMessage, is_valid_name};

use crate::net::{Connection, NetEvent};
use crate::{Screen, ServerAddress, Session};

const ACCENT: Color = Color::srgb(0.35, 0.85, 1.0);
const TEXT: Color = Color::srgb(0.88, 0.92, 1.0);
const FAINT: Color = Color::srgb(0.55, 0.62, 0.75);
const ERROR: Color = Color::srgb(1.0, 0.5, 0.45);

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MenuStatus>()
            .add_systems(OnEnter(Screen::Menu), spawn_menu)
            .add_systems(OnExit(Screen::Menu), despawn_menu)
            .add_systems(
                Update,
                (buttons, enter_key, show_status).run_if(in_state(Screen::Menu)),
            )
            // Server messages are read on every screen: the welcome arrives on the menu, a lost
            // connection while playing sends the player back to it.
            .add_systems(Update, receive);
    }
}

/// What the menu says under the button: nothing, connecting, or why it failed.
#[derive(Resource, Default)]
pub struct MenuStatus {
    pub text: String,
    pub error: bool,
    pub connecting: bool,
}

#[derive(Component)]
struct MenuRoot;
#[derive(Component)]
struct NameField;
#[derive(Component)]
struct ConnectButton;
#[derive(Component)]
struct DiceButton;
#[derive(Component)]
struct StatusText;

const CALLSIGNS: &[&str] = &[
    "Kestrel",
    "Vesper",
    "Halcyon",
    "Seraph",
    "Lumen",
    "Corvid",
    "Aster",
    "Nadir",
    "Zenith",
    "Tessera",
    "Orpheus",
    "Icarus",
    "Pioneer",
    "Mariner",
    "Wayfarer",
    "Ember",
    "Solace",
    "Cygnus",
    "Lyra",
    "Odyssey",
    "Perihelion",
    "Quasar",
    "Sentinel",
    "Tycho",
];

fn random_name() -> String {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let mut rng = Rng::new(seed);
    format!("{}-{}", rng.pick(CALLSIGNS), 10 + rng.next_u64() % 990)
}

fn label(text: &str, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(text),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

fn button(marker: impl Component, text: &str, primary: bool) -> impl Bundle {
    (
        marker,
        Button,
        Node {
            padding: UiRect::axes(Val::Px(22.0), Val::Px(10.0)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(6.0)),
            ..default()
        },
        BorderColor::all(if primary { ACCENT } else { FAINT }),
        BackgroundColor(if primary {
            Color::srgba(0.2, 0.6, 0.8, 0.35)
        } else {
            Color::srgba(1.0, 1.0, 1.0, 0.04)
        }),
        children![label(text, if primary { 20.0 } else { 16.0 }, TEXT)],
    )
}

fn spawn_menu(
    mut commands: Commands,
    mut focus: ResMut<InputFocus>,
    previous: Option<Res<LastName>>,
    server: Res<ServerAddress>,
    status: Res<MenuStatus>,
) {
    let name = previous.map(|n| n.0.clone()).unwrap_or_else(random_name);
    let mut field = Entity::PLACEHOLDER;
    commands
        .spawn((
            MenuRoot,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                row_gap: Val::Px(14.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn(label("PROJECT MMO", 54.0, TEXT));
            root.spawn(label(
                "A probe, sent from very far away, a very long time ago.",
                17.0,
                FAINT,
            ));
            root.spawn(Node {
                height: Val::Px(30.0),
                ..default()
            });
            root.spawn(label("PROBE DESIGNATION", 13.0, FAINT));
            root.spawn(Node {
                column_gap: Val::Px(10.0),
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                field = row
                    .spawn((
                        NameField,
                        EditableText {
                            max_characters: Some(MAX_NAME_CHARS),
                            allow_newlines: false,
                            visible_lines: Some(1.0),
                            ..EditableText::new(&name)
                        },
                        TextLayout::no_wrap(),
                        TextFont::from_font_size(24.0),
                        TextColor(TEXT),
                        Node {
                            width: Val::Px(320.0),
                            padding: UiRect::axes(Val::Px(14.0), Val::Px(9.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(6.0)),
                            ..default()
                        },
                        BorderColor::all(FAINT),
                        BackgroundColor(Color::srgba(0.05, 0.08, 0.14, 0.85)),
                    ))
                    .id();
                row.spawn(button(DiceButton, "Random", false));
            });
            root.spawn(button(ConnectButton, "WAKE UP", true));
            root.spawn((
                StatusText,
                label(
                    &status.text,
                    16.0,
                    if status.error { ERROR } else { ACCENT },
                ),
            ));
            root.spawn(label(&format!("galaxy server: {}", server.0), 13.0, FAINT));
        });
    focus.set(field, FocusCause::Navigated);
}

fn despawn_menu(
    mut commands: Commands,
    roots: Query<Entity, With<MenuRoot>>,
    mut status: ResMut<MenuStatus>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    status.connecting = false;
}

/// The name last used, offered again after a disconnection.
#[derive(Resource)]
struct LastName(String);

fn connect(commands: &mut Commands, status: &mut MenuStatus, server: &ServerAddress, name: &str) {
    if status.connecting {
        return;
    }
    let name = name.trim().to_string();
    if !is_valid_name(&name) {
        status.text = "A name is 3 to 24 letters, digits, - or _".into();
        status.error = true;
        return;
    }
    status.text = format!("Waking up {name}...");
    status.error = false;
    status.connecting = true;
    commands.insert_resource(LastName(name.clone()));
    commands.insert_resource(Connection::open(server.0.clone(), name));
}

#[allow(clippy::type_complexity)]
fn buttons(
    mut commands: Commands,
    mut status: ResMut<MenuStatus>,
    server: Res<ServerAddress>,
    pressed: Query<(&Interaction, Has<ConnectButton>, Has<DiceButton>), Changed<Interaction>>,
    mut fields: Query<&mut EditableText, With<NameField>>,
) {
    for (interaction, connect_button, dice) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        let Ok(mut field) = fields.single_mut() else {
            return;
        };
        if dice {
            *field = EditableText {
                max_characters: Some(MAX_NAME_CHARS),
                allow_newlines: false,
                visible_lines: Some(1.0),
                ..EditableText::new(random_name())
            };
        } else if connect_button {
            let name = field.value().to_string();
            connect(&mut commands, &mut status, &server, &name);
        }
    }
}

/// `--connect`: the menu connects with its random name as soon as it is up, once.
#[derive(Resource)]
pub struct AutoConnect(pub bool);

fn enter_key(
    mut commands: Commands,
    keys: Res<ButtonInput<KeyCode>>,
    mut auto: ResMut<AutoConnect>,
    mut status: ResMut<MenuStatus>,
    server: Res<ServerAddress>,
    fields: Query<&EditableText, With<NameField>>,
) {
    let auto = std::mem::replace(&mut auto.0, false);
    if (auto || keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter))
        && let Ok(field) = fields.single()
    {
        let name = field.value().to_string();
        connect(&mut commands, &mut status, &server, &name);
    }
}

fn show_status(
    status: Res<MenuStatus>,
    mut texts: Query<(&mut Text, &mut TextColor), With<StatusText>>,
) {
    if !status.is_changed() {
        return;
    }
    for (mut text, mut color) in &mut texts {
        text.0.clone_from(&status.text);
        color.0 = if status.error { ERROR } else { ACCENT };
    }
}

fn receive(
    mut commands: Commands,
    connection: Option<Res<Connection>>,
    mut status: ResMut<MenuStatus>,
    mut next: ResMut<NextState<Screen>>,
    screen: Res<State<Screen>>,
    time: Res<Time>,
    last_name: Option<Res<LastName>>,
) {
    let Some(connection) = connection else {
        return;
    };
    for event in connection.drain() {
        match event {
            NetEvent::Message(ServerMessage::Welcome(welcome)) => {
                status.text.clear();
                commands.insert_resource(Session {
                    name: last_name.as_ref().map(|n| n.0.clone()).unwrap_or_default(),
                    welcome,
                    started: time.elapsed_secs_f64(),
                });
                next.set(Screen::Playing);
            }
            NetEvent::Message(ServerMessage::Rejected { reason }) => {
                status.text = reason;
                status.error = true;
                status.connecting = false;
                commands.remove_resource::<Connection>();
            }
            NetEvent::Message(ServerMessage::Transfer { galaxy_id, .. }) => {
                // Linking galaxies comes later: for now a transfer is only reported.
                warn!("the server asked for a transfer to galaxy {galaxy_id}, not supported yet");
            }
            NetEvent::Lost(reason) => {
                status.text = reason;
                status.error = true;
                status.connecting = false;
                commands.remove_resource::<Connection>();
                if *screen.get() != Screen::Menu {
                    next.set(Screen::Menu);
                }
            }
        }
    }
}
