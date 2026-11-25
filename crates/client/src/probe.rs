//! The player's probe: its model, how it flies, the camera that follows it, and its position
//! reported to the server.
//!
//! The probe is three parts nested one inside the other, sharing a center:
//! - the **frame**, outside: the structure and the thrusters, three rings and six nozzles, one at
//!   each end of each axis, so it pushes the same in every direction;
//! - the **core**, inside the frame: the probe's thinking heart, a faceted shell that turns on
//!   itself. It is its own entity ([`ProbeCore`]) because it will detach from the frame in some
//!   situations;
//! - the **payload**, at the center of the core: the seed of life it carries.
//!
//! Space has no up: the camera turns freely on every axis, rolls included, and nothing in flight
//! points to a fixed vertical (only the building mode to come will have one).

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use mmo_protocol::ClientMessage;

use crate::net::Connection;
use crate::sky::MainCamera;
use crate::system::{PlanetBody, Star};
use crate::{InGame, Screen, Session};

/// Acceleration in units per second squared, and the speed it tops out at.
const THRUST: f32 = 40.0;
const MAX_SPEED: f32 = 120.0;
/// Shift: the long-range drive.
const BOOST: f32 = 6.0;
/// With nothing pressed, the probe's stabilizers bleed off this share of its speed per second.
const DRIFT_DAMPING: f32 = 0.5;
/// Radians per second the view rolls while Q or E (A or E on AZERTY) is held.
const ROLL_SPEED: f32 = 1.4;
const LOOK_SENSITIVITY: f32 = 0.005;
/// How often the probe's position goes to the server.
const REPORT_EVERY: f32 = 0.1;

/// The frame's radius: the rings, and the thrusters at the end of each axis.
const FRAME_RADIUS: f32 = 2.1;
const CORE_RADIUS: f32 = 0.95;
const PAYLOAD_RADIUS: f32 = 0.3;

const NOZZLE_IDLE: LinearRgba = LinearRgba::rgb(0.15, 0.35, 0.6);
const NOZZLE_FIRING: LinearRgba = LinearRgba::rgb(2.0, 6.0, 16.0);

pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Playing), spawn_probe)
            .add_systems(
                Update,
                (look, fly, collide, follow, report)
                    .chain()
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(
                Update,
                (turn_core, pulse_payload, fire_nozzles).run_if(in_state(Screen::Playing)),
            );
    }
}

#[derive(Component, Default)]
pub struct Probe {
    pub velocity: Vec3,
    /// Where the player pushes this frame, in world space, of length 0 to 1.
    pub push: Vec3,
}

/// The thinking heart: one day it leaves the frame on its own.
#[derive(Component)]
pub struct ProbeCore;

#[derive(Component)]
struct Payload;

/// A thruster at the end of an axis of the frame. It glows when it pushes the probe, that is when
/// the player pushes away from the side it stands on.
#[derive(Component)]
struct Nozzle {
    /// Which way it points, in the probe's own frame.
    outward: Vec3,
    material: Handle<StandardMaterial>,
    glow: f32,
}

/// The view around the probe: any orientation at all, and how far back.
#[derive(Resource)]
pub struct ProbeView {
    pub rotation: Quat,
    distance: f32,
}

#[derive(Resource)]
struct ReportTimer(Timer);

fn spawn_probe(
    mut commands: Commands,
    session: Res<Session>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let w = &session.welcome;
    let position = Vec3::from(w.spawn_position);
    // A new probe faces the star, tilted any which way; a returning one as it was left.
    let attitude = if w.first_arrival {
        let tilt = Vec3::new(0.45, 1.0, -0.6).normalize();
        Transform::from_translation(position)
            .looking_at(Vec3::ZERO, tilt)
            .rotation
    } else {
        Quat::from_array(w.spawn_rotation).normalize()
    };
    commands.insert_resource(ProbeView {
        // Three quarters from behind: the star ahead, the probe off to one side.
        rotation: attitude * Quat::from_rotation_y(0.7) * Quat::from_rotation_x(-0.3),
        distance: 12.0,
    });
    commands.insert_resource(ReportTimer(Timer::from_seconds(
        REPORT_EVERY,
        TimerMode::Repeating,
    )));

    let frame = materials.add(StandardMaterial {
        base_color: Color::srgb(0.78, 0.8, 0.86),
        emissive: LinearRgba::rgb(0.03, 0.04, 0.06),
        metallic: 0.6,
        perceptual_roughness: 0.4,
        ..default()
    });
    let seam = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.9, 1.0),
        emissive: LinearRgba::rgb(0.5, 2.4, 3.6),
        ..default()
    });
    let shell = materials.add(StandardMaterial {
        base_color: Color::srgba(0.35, 0.8, 1.0, 0.28),
        emissive: LinearRgba::rgb(0.05, 0.25, 0.45),
        alpha_mode: AlphaMode::Blend,
        double_sided: true,
        cull_mode: None,
        perceptual_roughness: 0.15,
        ..default()
    });
    let payload = materials.add(StandardMaterial {
        base_color: Color::srgb(0.6, 1.0, 0.7),
        emissive: LinearRgba::rgb(1.2, 6.0, 2.0),
        unlit: true,
        ..default()
    });

    let ring = meshes.add(Torus {
        minor_radius: 0.07,
        major_radius: FRAME_RADIUS,
    });
    let strut = meshes.add(Cylinder::new(0.035, FRAME_RADIUS - CORE_RADIUS - 0.25));
    let pod = meshes.add(Sphere::new(0.26).mesh().ico(0).expect("fine"));
    let nozzle = meshes.add(Cone {
        radius: 0.2,
        height: 0.32,
    });
    let core_shell = meshes.add(Sphere::new(CORE_RADIUS).mesh().ico(1).expect("fine"));
    let core_band = meshes.add(Torus {
        minor_radius: 0.03,
        major_radius: CORE_RADIUS * 1.02,
    });
    let seed = meshes.add(Sphere::new(PAYLOAD_RADIUS).mesh().ico(3).expect("fine"));

    commands
        .spawn((
            InGame,
            Probe::default(),
            Transform::from_translation(position).with_rotation(attitude),
            Visibility::default(),
        ))
        .with_children(|probe| {
            // The frame: three rings at right angles...
            for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
                probe.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(frame.clone()),
                    Transform::from_rotation(Quat::from_rotation_arc(Vec3::Y, axis)),
                ));
            }
            // ...and at the six points where they cross, the thrusters, each held to the core
            // by a strut.
            for outward in [
                Vec3::X,
                Vec3::NEG_X,
                Vec3::Y,
                Vec3::NEG_Y,
                Vec3::Z,
                Vec3::NEG_Z,
            ] {
                let turn = Quat::from_rotation_arc(Vec3::Y, outward);
                let material = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.4, 0.7, 1.0),
                    emissive: NOZZLE_IDLE,
                    unlit: true,
                    ..default()
                });
                probe.spawn((
                    Mesh3d(strut.clone()),
                    MeshMaterial3d(frame.clone()),
                    Transform::from_translation(outward * (CORE_RADIUS + FRAME_RADIUS) * 0.5)
                        .with_rotation(turn),
                ));
                probe.spawn((
                    Mesh3d(pod.clone()),
                    MeshMaterial3d(frame.clone()),
                    Transform::from_translation(outward * FRAME_RADIUS).with_rotation(turn),
                ));
                probe.spawn((
                    Nozzle {
                        outward,
                        material: material.clone(),
                        glow: 0.0,
                    },
                    Mesh3d(nozzle.clone()),
                    MeshMaterial3d(material),
                    Transform::from_translation(outward * (FRAME_RADIUS + 0.3))
                        .with_rotation(Quat::from_rotation_arc(Vec3::NEG_Y, outward)),
                ));
            }
            // The core, turning inside the frame, with the payload at its heart.
            probe
                .spawn((ProbeCore, Transform::default(), Visibility::default()))
                .with_children(|core| {
                    core.spawn((Mesh3d(core_shell), MeshMaterial3d(shell)));
                    for tilt in [0.0, 1.1] {
                        core.spawn((
                            Mesh3d(core_band.clone()),
                            MeshMaterial3d(seam.clone()),
                            Transform::from_rotation(Quat::from_rotation_x(tilt)),
                        ));
                    }
                    core.spawn((Payload, Mesh3d(seed), MeshMaterial3d(payload)));
                });
        });
}

/// Drag with either mouse button to turn the view on its own axes; Q and E (A and E on AZERTY)
/// roll it; the wheel brings it closer. Nothing holds it level: there is no level.
fn look(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut view: ResMut<ProbeView>,
) {
    let mut turn = Quat::IDENTITY;
    if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
        turn = Quat::from_rotation_y(-motion.delta.x * LOOK_SENSITIVITY)
            * Quat::from_rotation_x(-motion.delta.y * LOOK_SENSITIVITY);
    }
    // Physical keys: Q and E on QWERTY are A and E on AZERTY.
    let mut roll = 0.0;
    if keys.pressed(KeyCode::KeyQ) {
        roll += 1.0;
    }
    if keys.pressed(KeyCode::KeyE) {
        roll -= 1.0;
    }
    turn *= Quat::from_rotation_z(roll * ROLL_SPEED * time.delta_secs());
    view.rotation = (view.rotation * turn).normalize();

    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    view.distance = (view.distance * 0.9f32.powf(notches)).clamp(5.0, 120.0);
}

fn fly(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    view: Res<ProbeView>,
    mut probe: Query<(&mut Probe, &mut Transform)>,
) {
    let Ok((mut probe, mut transform)) = probe.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let forward = view.rotation * Vec3::NEG_Z;
    let right = view.rotation * Vec3::X;
    let up = view.rotation * Vec3::Y;

    // Physical keys: WASD on QWERTY is ZQSD on AZERTY.
    let pressed = |codes: &[KeyCode]| codes.iter().any(|c| keys.pressed(*c));
    let mut input = Vec3::ZERO;
    if pressed(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
        input += forward;
    }
    if pressed(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
        input -= forward;
    }
    if pressed(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
        input += right;
    }
    if pressed(&[KeyCode::KeyA, KeyCode::ArrowLeft]) {
        input -= right;
    }
    if pressed(&[KeyCode::Space]) {
        input += up;
    }
    if pressed(&[KeyCode::ControlLeft, KeyCode::KeyC]) {
        input -= up;
    }
    let boost = if pressed(&[KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        BOOST
    } else {
        1.0
    };
    let braking = pressed(&[KeyCode::KeyX]);

    let input = input.normalize_or_zero();
    probe.push = input;
    probe.velocity += input * THRUST * boost * dt;
    let top = MAX_SPEED * boost;
    let speed = probe.velocity.length();
    if speed > top {
        // Above the top speed (the boost just released), ease down instead of stopping dead.
        probe.velocity *= (top / speed).max((-1.5 * dt).exp());
    }
    if input == Vec3::ZERO || braking {
        let damping = if braking { 3.0 } else { DRIFT_DAMPING };
        probe.velocity *= (-damping * dt).exp();
    }
    let velocity = probe.velocity;
    // The probe keeps its attitude: it pushes the same way on every axis, so it never needs to
    // turn, and the view can circle it freely.
    transform.translation += velocity * dt;
}

/// The core turns on itself, on a slanted axis.
fn turn_core(time: Res<Time>, mut cores: Query<&mut Transform, With<ProbeCore>>) {
    let axis = Vec3::new(0.3, 1.0, 0.2).normalize();
    for mut core in &mut cores {
        core.rotate_local(Quat::from_axis_angle(axis, 0.5 * time.delta_secs()));
    }
}

/// The payload beats, slowly, like something asleep.
fn pulse_payload(time: Res<Time>, mut seeds: Query<&mut Transform, With<Payload>>) {
    let beat = 1.0 + 0.08 * (time.elapsed_secs() * 1.6).sin();
    for mut seed in &mut seeds {
        seed.scale = Vec3::splat(beat);
    }
}

/// A thruster glows when it pushes: when the player pushes away from its side of the frame.
fn fire_nozzles(
    time: Res<Time>,
    probe: Query<(&Probe, &Transform)>,
    mut nozzles: Query<(&mut Nozzle, &mut Transform), Without<Probe>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((probe, attitude)) = probe.single() else {
        return;
    };
    let ease = (time.delta_secs() * 8.0).min(1.0);
    for (mut nozzle, mut transform) in &mut nozzles {
        let outward = attitude.rotation * nozzle.outward;
        let wanted = (-outward.dot(probe.push)).max(0.0);
        nozzle.glow += (wanted - nozzle.glow) * ease;
        transform.scale = Vec3::new(1.0, 1.0 + nozzle.glow * 2.5, 1.0);
        if let Some(mut material) = materials.get_mut(&nozzle.material) {
            material.emissive = NOZZLE_IDLE + (NOZZLE_FIRING - NOZZLE_IDLE) * nozzle.glow;
        }
    }
}

/// The probe cannot fly through the star or a planet.
fn collide(
    mut probe: Query<(&mut Probe, &mut Transform)>,
    stars: Query<(&Star, &Transform), Without<Probe>>,
    planets: Query<(&PlanetBody, &Transform), Without<Probe>>,
) {
    let Ok((mut probe, mut transform)) = probe.single_mut() else {
        return;
    };
    let bodies = stars
        .iter()
        .map(|(s, t)| (t.translation, s.radius * 1.15))
        .chain(
            planets
                .iter()
                .map(|(p, t)| (t.translation, p.0.radius + 4.0)),
        );
    for (center, radius) in bodies {
        let offset = transform.translation - center;
        let distance = offset.length();
        if distance < radius && distance > 0.0 {
            let normal = offset / distance;
            transform.translation = center + normal * radius;
            let into = probe.velocity.dot(normal);
            if into < 0.0 {
                probe.velocity -= normal * into;
            }
        }
    }
}

fn follow(
    view: Res<ProbeView>,
    probe: Query<&Transform, (With<Probe>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let (Ok(probe), Ok(mut camera)) = (probe.single(), camera.single_mut()) else {
        return;
    };
    camera.rotation = view.rotation;
    camera.translation =
        probe.translation + view.rotation * Vec3::new(0.0, view.distance * 0.12, view.distance);
}

fn report(
    time: Res<Time>,
    mut timer: ResMut<ReportTimer>,
    connection: Option<Res<Connection>>,
    probe: Query<&Transform, With<Probe>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if let (Some(connection), Ok(probe)) = (connection, probe.single()) {
        connection.send(ClientMessage::ProbeState {
            position: probe.translation.to_array(),
            rotation: probe.rotation.to_array(),
        });
    }
}
