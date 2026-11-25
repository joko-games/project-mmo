//! What is drawn over the game: the fade from black on arrival, the wake-up log of a first
//! arrival, the probe's readouts, and the names of the worlds in view.

use bevy::prelude::*;
use mmo_protocol::{ClientMessage, PlanetKind};

use crate::net::Connection;
use crate::probe::Probe;
use crate::sky::MainCamera;
use crate::system::PlanetBody;
use crate::{InGame, Screen, Session};

const TEXT: Color = Color::srgb(0.85, 0.92, 1.0);
const FAINT: Color = Color::srgb(0.5, 0.6, 0.75);
const ACCENT: Color = Color::srgb(0.35, 0.85, 1.0);
const LIFE: Color = Color::srgb(0.5, 1.0, 0.6);

/// The wake-up log of a first arrival: each line, when it appears and for how long.
const WAKE_UP: &[(f32, f32, &str)] = &[
    (2.0, 4.5, "...signal. Systems waking."),
    (
        7.0,
        5.0,
        "Cruise phase complete. Time in transit: 41,206 years.",
    ),
    (
        12.5,
        5.0,
        "Origin: beyond the edge of any map you will ever read.",
    ),
    (18.0, 5.5, "Destination reached. A young star. Unclaimed."),
    (
        24.0,
        6.0,
        "Primary directive: find a world where life can take root. Deliver the payload.",
    ),
    (
        30.5,
        5.5,
        "Until then, this system is yours. Build. Explore. Take your time.",
    ),
];
/// How long the screen takes to come out of black: the long fade of a first arrival, a short
/// one when coming back.
const FIRST_FADE: f32 = 5.0;
const RETURN_FADE: f32 = 1.2;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        // After the system is spawned: there is a label for each of its planets.
        app.add_systems(
            OnEnter(Screen::Playing),
            spawn_hud.after(crate::system::spawn_system),
        )
        .add_systems(
            Update,
            (fade_in, wake_up_log, readouts, planet_labels).run_if(in_state(Screen::Playing)),
        );
    }
}

#[derive(Component)]
struct Fade;
#[derive(Component)]
struct WakeLine;
#[derive(Component)]
struct Readout;
#[derive(Component)]
struct Nearest;
#[derive(Component)]
struct Speed;
#[derive(Component)]
struct PlanetLabel(Entity);

/// Whether the server was told the wake-up log was seen.
#[derive(Resource)]
struct WakeUpDone(bool);

fn text(value: impl Into<String>, size: f32, color: Color) -> impl Bundle {
    (
        Text::new(value),
        TextFont::from_font_size(size),
        TextColor(color),
    )
}

fn spawn_hud(mut commands: Commands, session: Res<Session>, planets: Query<(Entity, &PlanetBody)>) {
    let w = &session.welcome;
    commands.insert_resource(WakeUpDone(!w.first_arrival));

    // Top left: who and where.
    commands.spawn((
        InGame,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(18.0),
            top: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(3.0),
            ..default()
        },
        children![
            text(
                format!("PROBE {}", session.name.to_uppercase()),
                20.0,
                ACCENT
            ),
            text(format!("Galaxy  {}", w.galaxy.name), 14.0, TEXT),
            text(
                format!("System  {}  (home #{})", w.home.name, w.home.index + 1),
                14.0,
                TEXT
            ),
            text(
                format!(
                    "{} probe(s) woken in this galaxy. The nearest is light years away.",
                    w.probes_in_galaxy
                ),
                12.0,
                FAINT
            ),
        ],
    ));

    // Top right: the directive.
    commands.spawn((
        InGame,
        Node {
            position_type: PositionType::Absolute,
            right: Val::Px(18.0),
            top: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::End,
            row_gap: Val::Px(3.0),
            ..default()
        },
        children![
            text("PRIMARY DIRECTIVE", 12.0, FAINT),
            text("Find a world where life can take root", 15.0, LIFE),
            text("Payload: dormant, intact", 12.0, FAINT),
            (Nearest, text("", 13.0, TEXT)),
        ],
    ));

    // Bottom: speed and controls.
    commands.spawn((
        InGame,
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(18.0),
            bottom: Val::Px(14.0),
            flex_direction: FlexDirection::Column,
            row_gap: Val::Px(4.0),
            ..default()
        },
        children![
            (Speed, Readout, text("", 18.0, TEXT)),
            text(
                "WASD/ZQSD move   |   Space / Ctrl up, down   |   Shift long-range drive   |   X brake   |   drag to look   |   wheel zoom",
                12.0,
                FAINT
            ),
        ],
    ));

    for (entity, planet) in &planets {
        commands.spawn((
            InGame,
            PlanetLabel(entity),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            text(
                planet.0.name.clone(),
                13.0,
                if planet.0.kind == PlanetKind::Viable {
                    LIFE
                } else {
                    FAINT
                },
            ),
            Visibility::Hidden,
        ));
    }

    // The wake-up log, low in the middle.
    commands.spawn((
        InGame,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            bottom: Val::Percent(22.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        children![(WakeLine, text("", 22.0, TEXT.with_alpha(0.0)))],
    ));

    // Black over everything, fading away.
    commands.spawn((
        InGame,
        Fade,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::BLACK),
        GlobalZIndex(10),
    ));
}

fn since_arrival(time: &Time, session: &Session) -> f32 {
    (time.elapsed_secs_f64() - session.started) as f32
}

fn fade_in(
    time: Res<Time>,
    session: Res<Session>,
    mut fade: Query<(&mut BackgroundColor, &mut Visibility), With<Fade>>,
) {
    let length = if session.welcome.first_arrival {
        FIRST_FADE
    } else {
        RETURN_FADE
    };
    let t = (since_arrival(&time, &session) / length).clamp(0.0, 1.0);
    for (mut color, mut visibility) in &mut fade {
        // Slow at first, as eyes adjusting.
        color.0 = Color::BLACK.with_alpha(1.0 - t * t * (3.0 - 2.0 * t));
        if t >= 1.0 {
            *visibility = Visibility::Hidden;
        }
    }
}

fn wake_up_log(
    time: Res<Time>,
    session: Res<Session>,
    mut done: ResMut<WakeUpDone>,
    connection: Option<Res<Connection>>,
    mut lines: Query<(&mut Text, &mut TextColor), With<WakeLine>>,
) {
    if !session.welcome.first_arrival {
        return;
    }
    let t = since_arrival(&time, &session);
    let Ok((mut text, mut color)) = lines.single_mut() else {
        return;
    };
    let current = WAKE_UP
        .iter()
        .find(|(start, length, _)| t >= *start && t < start + length);
    match current {
        Some((start, length, line)) => {
            if text.0 != *line {
                text.0 = line.to_string();
            }
            let local = t - start;
            let alpha = (local / 0.8)
                .min(1.0)
                .min((length - local) / 0.8)
                .clamp(0.0, 1.0);
            color.0 = TEXT.with_alpha(alpha);
        }
        None => color.0 = TEXT.with_alpha(0.0),
    }
    let (last_start, last_length, _) = WAKE_UP[WAKE_UP.len() - 1];
    if !done.0 && t > last_start + last_length {
        done.0 = true;
        if let Some(connection) = connection {
            connection.send(ClientMessage::IntroSeen);
        }
    }
}

fn readouts(
    probe: Query<(&Probe, &Transform)>,
    planets: Query<(&PlanetBody, &Transform)>,
    mut speed: Query<&mut Text, (With<Speed>, Without<Nearest>)>,
    mut nearest: Query<&mut Text, (With<Nearest>, Without<Speed>)>,
) {
    let Ok((probe, transform)) = probe.single() else {
        return;
    };
    if let Ok(mut text) = speed.single_mut() {
        text.0 = format!("{:>6.1} u/s", probe.velocity.length());
    }
    let closest = planets
        .iter()
        .map(|(p, t)| {
            (
                p,
                t.translation.distance(transform.translation) - p.0.radius,
            )
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    if let (Ok(mut text), Some((planet, distance))) = (nearest.single_mut(), closest) {
        text.0 = format!(
            "Nearest world: {} ({})  {:.0} u",
            planet.0.name,
            planet.0.kind.label(),
            distance.max(0.0)
        );
    }
}

fn planet_labels(
    camera: Query<(&Camera, &GlobalTransform), With<MainCamera>>,
    planets: Query<(&PlanetBody, &GlobalTransform)>,
    probe: Query<&Transform, With<Probe>>,
    mut labels: Query<(&PlanetLabel, &mut Node, &mut Visibility, &mut Text)>,
) {
    let (Ok((camera, camera_transform)), Ok(probe)) = (camera.single(), probe.single()) else {
        return;
    };
    for (label, mut node, mut visibility, mut text) in &mut labels {
        let Ok((planet, transform)) = planets.get(label.0) else {
            continue;
        };
        let top = transform.translation() + Vec3::Y * planet.0.radius * 1.3;
        let in_front = camera_transform
            .forward()
            .dot(top - camera_transform.translation())
            > 0.0;
        match camera.world_to_viewport(camera_transform, top) {
            Ok(at) if in_front => {
                *visibility = Visibility::Inherited;
                node.left = Val::Px(at.x - 40.0);
                node.top = Val::Px(at.y - 22.0);
                let distance =
                    probe.translation.distance(transform.translation()) - planet.0.radius;
                let wanted = format!("{}  {:.0} u", planet.0.name, distance.max(0.0));
                if text.0 != wanted {
                    text.0 = wanted;
                }
            }
            _ => *visibility = Visibility::Hidden,
        }
    }
}
