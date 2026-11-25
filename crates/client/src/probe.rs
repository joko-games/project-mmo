//! The player's probe: its model, how it flies, the views that follow it, and its position
//! reported to the server.
//!
//! The probe is three parts nested one inside the other, its whole shape a drop pulled backwards:
//! - the **structure**, outside: ribs and hoops swept back from a round nose to a pointed tail,
//!   the main engine at the tail and small thrusters around the nose;
//! - the **core**, inside: the probe's thinking heart. It detaches: Tab sends the player into the
//!   core, which flies on its own with a simpler navigation, the one building will use;
//! - the **payload**, at the center of the core: the seed of life it carries.
//!
//! Flying the structure, space has no up: the view turns freely on every axis, rolls included.
//! Flying the core, the structure gives the reference: up is the structure's up, as it will be
//! when building on it.

use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::prelude::*;
use mmo_protocol::ClientMessage;

use crate::net::Connection;
use crate::shapes::{Drop, lathe, tube};
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

/// The core flies at this speed, three times faster with Shift, and stops when nothing is
/// pressed. It stays within this distance of the structure.
const CORE_SPEED: f32 = 18.0;
const CORE_LEASH: f32 = 250.0;

/// The structure's shape.
const HULL: Drop = Drop {
    nose: 1.9,
    tail: 6.5,
    radius: 1.9,
    tail_radius: 0.28,
};
/// The core's shape: the same drop, smaller.
const CORE: Drop = Drop {
    nose: 0.75,
    tail: 2.1,
    radius: 0.8,
    tail_radius: 0.05,
};
const PAYLOAD_RADIUS: f32 = 0.26;

const NOZZLE_IDLE: LinearRgba = LinearRgba::rgb(0.15, 0.35, 0.6);
const NOZZLE_FIRING: LinearRgba = LinearRgba::rgb(2.0, 6.0, 16.0);

pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Playing), spawn_probe)
            .add_systems(
                Update,
                (switch, look, fly, fly_core, collide, follow, report)
                    .chain()
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(
                Update,
                (spin_core, pulse_payload, fire_nozzles).run_if(in_state(Screen::Playing)),
            );
    }
}

/// What the player flies: the whole probe, or the core alone.
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Piloting {
    Structure,
    Core,
}

/// The structure, and the probe as the server knows it.
#[derive(Component, Default)]
pub struct Probe {
    pub velocity: Vec3,
    /// Where the player pushes this frame, in world space, of length 0 to 1.
    pub push: Vec3,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CoreState {
    /// In its place inside the structure, moving with it.
    Docked,
    /// Out, flown by the player.
    Free,
    /// Flying itself back into the structure.
    Returning,
}

/// The thinking heart.
#[derive(Component)]
pub struct Core {
    pub state: CoreState,
    pub velocity: Vec3,
}

/// The part of the core that turns on itself.
#[derive(Component)]
struct CoreSpin;

#[derive(Component)]
struct Payload;

/// A thruster on the structure. It glows when it pushes, that is when the player pushes away
/// from the way it points.
#[derive(Component)]
struct Nozzle {
    /// Which way it points, in the structure's own frame.
    outward: Vec3,
    material: Handle<StandardMaterial>,
    glow: f32,
    /// How long its flame grows at full thrust.
    flame: f32,
}

/// The view around the structure: any orientation at all, and how far back.
#[derive(Resource)]
pub struct ProbeView {
    pub rotation: Quat,
    distance: f32,
}

/// The view around the core: turning and tilting against the structure's axes, never rolling.
#[derive(Resource)]
pub struct CoreView {
    pub(crate) yaw: f32,
    pub(crate) pitch: f32,
    pub(crate) distance: f32,
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
    commands.insert_resource(Piloting::Structure);
    commands.insert_resource(ProbeView {
        // Three quarters from behind: the star ahead, the probe's long side in view.
        rotation: attitude * Quat::from_rotation_y(0.6) * Quat::from_rotation_x(-0.25),
        distance: 16.0,
    });
    commands.insert_resource(CoreView {
        yaw: 0.0,
        pitch: 0.0,
        distance: 5.0,
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
        base_color: Color::srgba(0.35, 0.8, 1.0, 0.3),
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

    // The ribs: six lines from the nose to the tail along the drop's surface, each swept around
    // the axis as it goes, so the structure seems drawn backwards.
    let rib_path = |angle: f32| -> Vec<Vec3> {
        HULL.profile(40)
            .into_iter()
            .map(|(z, r)| {
                let twist = angle + 0.25 * (z / HULL.tail).max(0.0);
                Vec3::new(r * twist.cos(), r * twist.sin(), z)
            })
            .collect()
    };
    let ribs: Vec<_> = (0..6)
        .map(|k| {
            meshes.add(tube(
                &rib_path(k as f32 * std::f32::consts::TAU / 6.0),
                0.06,
                6,
            ))
        })
        .collect();
    let hoop = |z: f32, meshes: &mut Assets<Mesh>| {
        meshes.add(Torus {
            minor_radius: 0.07,
            major_radius: HULL.radius_at(z),
        })
    };
    let hoops: Vec<_> = [-1.2, 0.0, 2.2, 4.4]
        .into_iter()
        .map(|z| (z, hoop(z, &mut meshes)))
        .collect();
    let small_nozzle = meshes.add(Cone {
        radius: 0.16,
        height: 0.28,
    });
    let engine_bell = meshes.add(Cone {
        radius: 0.42,
        height: 0.6,
    });
    let pod = meshes.add(Sphere::new(0.18).mesh().ico(0).expect("fine"));
    let core_shell = meshes.add(lathe(&CORE.profile(28), 24));
    let core_band = meshes.add(Torus {
        minor_radius: 0.025,
        major_radius: CORE.radius * 1.02,
    });
    let seed = meshes.add(Sphere::new(PAYLOAD_RADIUS).mesh().ico(3).expect("fine"));

    // Hoops lie across the axis: the torus's own axis (Y) turned onto Z.
    let across = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    let mut nozzles = Vec::new();
    // The main engine at the tail, pointing back.
    nozzles.push((Vec3::Z, Vec3::Z * (HULL.tail + 0.25), engine_bell, 9.0));
    // Thrusters around the nose hoop and the tail hoop, pointing out, for every other direction.
    for (z, count) in [(-1.2f32, 4), (4.4, 4)] {
        let r = HULL.radius_at(z);
        for k in 0..count {
            let a = k as f32 * std::f32::consts::TAU / count as f32 + 0.4;
            let out = Vec3::new(a.cos(), a.sin(), 0.0);
            nozzles.push((
                out,
                out * (r + 0.22) + Vec3::Z * z,
                small_nozzle.clone(),
                2.5,
            ));
        }
    }
    // And one at the nose, pointing forward, to brake and back off.
    nozzles.push((
        Vec3::NEG_Z,
        Vec3::NEG_Z * (HULL.nose + 0.2),
        small_nozzle.clone(),
        2.5,
    ));

    commands
        .spawn((
            InGame,
            Probe::default(),
            Transform::from_translation(position).with_rotation(attitude),
            Visibility::default(),
        ))
        .with_children(|probe| {
            for rib in ribs {
                probe.spawn((Mesh3d(rib), MeshMaterial3d(frame.clone())));
            }
            for (z, mesh) in hoops {
                probe.spawn((
                    Mesh3d(mesh),
                    MeshMaterial3d(if z == 0.0 {
                        seam.clone()
                    } else {
                        frame.clone()
                    }),
                    Transform::from_xyz(0.0, 0.0, z).with_rotation(across),
                ));
            }
            for (outward, at, mesh, flame) in nozzles {
                let material = materials.add(StandardMaterial {
                    base_color: Color::srgb(0.4, 0.7, 1.0),
                    emissive: NOZZLE_IDLE,
                    unlit: true,
                    ..default()
                });
                probe.spawn((
                    Mesh3d(pod.clone()),
                    MeshMaterial3d(frame.clone()),
                    Transform::from_translation(at - outward * 0.15),
                ));
                probe.spawn((
                    Nozzle {
                        outward,
                        material: material.clone(),
                        glow: 0.0,
                        flame,
                    },
                    Mesh3d(mesh),
                    MeshMaterial3d(material),
                    // The cone's apex is +Y: turned to point inward, its bell opens outward.
                    Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_arc(Vec3::NEG_Y, outward)),
                ));
            }
        });

    // The core is its own entity, carried by the structure while docked.
    commands
        .spawn((
            InGame,
            Core {
                state: CoreState::Docked,
                velocity: Vec3::ZERO,
            },
            Transform::from_translation(position).with_rotation(attitude),
            Visibility::default(),
        ))
        .with_children(|core| {
            core.spawn((CoreSpin, Transform::default(), Visibility::default()))
                .with_children(|spin| {
                    spin.spawn((Mesh3d(core_shell), MeshMaterial3d(shell)));
                    for tilt in [0.0, 0.9] {
                        spin.spawn((
                            Mesh3d(core_band.clone()),
                            MeshMaterial3d(seam.clone()),
                            Transform::from_rotation(across * Quat::from_rotation_z(tilt)),
                        ));
                    }
                });
            core.spawn((
                Payload,
                Mesh3d(seed),
                MeshMaterial3d(payload),
                Transform::default(),
            ));
        });
}

/// Tab: out into the core, or back into the structure.
fn switch(
    keys: Res<ButtonInput<KeyCode>>,
    mut piloting: ResMut<Piloting>,
    mut core_view: ResMut<CoreView>,
    view: Res<ProbeView>,
    probe: Query<&Transform, With<Probe>>,
    mut cores: Query<&mut Core>,
) {
    if !keys.just_pressed(KeyCode::Tab) {
        return;
    }
    let (Ok(structure), Ok(mut core)) = (probe.single(), cores.single_mut()) else {
        return;
    };
    match *piloting {
        Piloting::Structure => {
            *piloting = Piloting::Core;
            core.state = CoreState::Free;
            core.velocity = Vec3::ZERO;
            // Looking the way the probe's view looked, as far as the structure's axes allow.
            let relative = structure.rotation.inverse() * view.rotation;
            let forward = relative * Vec3::NEG_Z;
            core_view.yaw = (-forward.x).atan2(-forward.z);
            core_view.pitch = forward.y.clamp(-1.0, 1.0).asin().clamp(-1.4, 1.4);
        }
        Piloting::Core => {
            *piloting = Piloting::Structure;
            core.state = CoreState::Returning;
        }
    }
}

/// Flying the structure: drag to turn the view on its own axes, Q and E (A and E on AZERTY) to
/// roll it; nothing holds it level, there is no level. Flying the core: drag to turn and tilt
/// against the structure's axes. The wheel brings either view closer.
#[allow(clippy::too_many_arguments)]
fn look(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    piloting: Res<Piloting>,
    mut view: ResMut<ProbeView>,
    mut core_view: ResMut<CoreView>,
) {
    let dragging = buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right);
    let delta = if dragging { motion.delta } else { Vec2::ZERO };
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    let zoom = 0.9f32.powf(notches);

    match *piloting {
        Piloting::Structure => {
            let mut roll = 0.0;
            // Physical keys: Q and E on QWERTY are A and E on AZERTY.
            if keys.pressed(KeyCode::KeyQ) {
                roll += 1.0;
            }
            if keys.pressed(KeyCode::KeyE) {
                roll -= 1.0;
            }
            let turn = Quat::from_rotation_y(-delta.x * LOOK_SENSITIVITY)
                * Quat::from_rotation_x(-delta.y * LOOK_SENSITIVITY)
                * Quat::from_rotation_z(roll * ROLL_SPEED * time.delta_secs());
            view.rotation = (view.rotation * turn).normalize();
            view.distance = (view.distance * zoom).clamp(7.0, 140.0);
        }
        Piloting::Core => {
            core_view.yaw -= delta.x * LOOK_SENSITIVITY;
            core_view.pitch = (core_view.pitch - delta.y * LOOK_SENSITIVITY).clamp(-1.4, 1.4);
            core_view.distance = (core_view.distance * zoom).clamp(2.5, 40.0);
        }
    }
}

/// The keys as a direction in the view's own axes: x right, y up, z back.
fn keys_direction(keys: &ButtonInput<KeyCode>) -> Vec3 {
    // Physical keys: WASD on QWERTY is ZQSD on AZERTY.
    let pressed = |codes: &[KeyCode]| codes.iter().any(|c| keys.pressed(*c));
    let mut input = Vec3::ZERO;
    if pressed(&[KeyCode::KeyW, KeyCode::ArrowUp]) {
        input.z -= 1.0;
    }
    if pressed(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
        input.z += 1.0;
    }
    if pressed(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
        input.x += 1.0;
    }
    if pressed(&[KeyCode::KeyA, KeyCode::ArrowLeft]) {
        input.x -= 1.0;
    }
    if pressed(&[KeyCode::Space]) {
        input.y += 1.0;
    }
    if pressed(&[KeyCode::ControlLeft, KeyCode::KeyC]) {
        input.y -= 1.0;
    }
    input
}

fn boosting(keys: &ButtonInput<KeyCode>) -> bool {
    keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight)
}

fn fly(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    view: Res<ProbeView>,
    piloting: Res<Piloting>,
    mut probe: Query<(&mut Probe, &mut Transform)>,
) {
    let Ok((mut probe, mut transform)) = probe.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let flying = *piloting == Piloting::Structure;
    let input = if flying {
        (view.rotation * keys_direction(&keys)).normalize_or_zero()
    } else {
        Vec3::ZERO
    };
    let boost = if flying && boosting(&keys) {
        BOOST
    } else {
        1.0
    };
    let braking = flying && keys.pressed(KeyCode::KeyX);

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
    transform.translation += velocity * dt;

    // Under thrust the probe turns its nose where it pushes, keeping the view's roll.
    if input != Vec3::ZERO {
        let up = view.rotation * Vec3::Y;
        let target = Transform::default().looking_to(input, up).rotation;
        transform.rotation = transform.rotation.slerp(target, (dt * 1.8).min(1.0));
    }
}

/// The core: carried while docked, flown while free, flying itself home while returning.
///
/// Free, it moves like a builder: no inertia, forward and sideways in the plane of the
/// structure's axes, Space and Ctrl along the structure's up.
fn fly_core(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    core_view: Res<CoreView>,
    probe: Query<&Transform, (With<Probe>, Without<Core>)>,
    mut cores: Query<(&mut Core, &mut Transform)>,
) {
    let (Ok(structure), Ok((mut core, mut transform))) = (probe.single(), cores.single_mut())
    else {
        return;
    };
    let dt = time.delta_secs();
    match core.state {
        CoreState::Docked => {
            *transform = structure.with_scale(Vec3::ONE);
        }
        CoreState::Free => {
            let heading = structure.rotation * Quat::from_rotation_y(core_view.yaw);
            let speed = CORE_SPEED * if boosting(&keys) { 3.0 } else { 1.0 };
            let wanted = heading * keys_direction(&keys).normalize_or_zero() * speed;
            core.velocity = core.velocity.lerp(wanted, (dt * 10.0).min(1.0));
            let velocity = core.velocity;
            transform.translation += velocity * dt;
            let offset = transform.translation - structure.translation;
            if offset.length() > CORE_LEASH {
                transform.translation = structure.translation + offset.normalize() * CORE_LEASH;
            }
            transform.rotation = transform.rotation.slerp(heading, (dt * 6.0).min(1.0));
        }
        CoreState::Returning => {
            let offset = structure.translation - transform.translation;
            let distance = offset.length();
            let step = (30.0 + distance * 3.0) * dt;
            if distance <= step.max(0.05) {
                core.state = CoreState::Docked;
                core.velocity = Vec3::ZERO;
                *transform = structure.with_scale(Vec3::ONE);
            } else {
                transform.translation += offset / distance * step;
                transform.rotation = transform
                    .rotation
                    .slerp(structure.rotation, (dt * 4.0).min(1.0));
            }
        }
    }
}

/// The core turns on its long axis.
fn spin_core(time: Res<Time>, mut spins: Query<&mut Transform, With<CoreSpin>>) {
    for mut spin in &mut spins {
        spin.rotate_local_z(0.6 * time.delta_secs());
    }
}

/// The payload beats, slowly, like something asleep.
fn pulse_payload(time: Res<Time>, mut seeds: Query<&mut Transform, With<Payload>>) {
    let beat = 1.0 + 0.08 * (time.elapsed_secs() * 1.6).sin();
    for mut seed in &mut seeds {
        seed.scale = Vec3::new(beat, beat, beat * 1.25);
    }
}

/// A thruster glows, and its flame grows, when it pushes: when the player pushes away from the
/// way it points.
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
        transform.scale = Vec3::new(1.0, 1.0 + nozzle.glow * nozzle.flame, 1.0);
        if let Some(mut material) = materials.get_mut(&nozzle.material) {
            material.emissive = NOZZLE_IDLE + (NOZZLE_FIRING - NOZZLE_IDLE) * nozzle.glow;
        }
    }
}

/// Pushes a point out of every body it entered, and cancels its speed into them.
fn push_out(position: &mut Vec3, velocity: &mut Vec3, bodies: impl Iterator<Item = (Vec3, f32)>) {
    for (center, radius) in bodies {
        let offset = *position - center;
        let distance = offset.length();
        if distance < radius && distance > 0.0 {
            let normal = offset / distance;
            *position = center + normal * radius;
            let into = velocity.dot(normal);
            if into < 0.0 {
                *velocity -= normal * into;
            }
        }
    }
}

/// Neither the probe nor its core can fly through the star or a planet.
#[allow(clippy::type_complexity)]
fn collide(
    mut probe: Query<(&mut Probe, &mut Transform), Without<Core>>,
    mut cores: Query<(&mut Core, &mut Transform), Without<Probe>>,
    stars: Query<(&Star, &Transform), (Without<Probe>, Without<Core>)>,
    planets: Query<(&PlanetBody, &Transform), (Without<Probe>, Without<Core>)>,
) {
    let bodies = || {
        stars
            .iter()
            .map(|(s, t)| (t.translation, s.radius * 1.15))
            .chain(
                planets
                    .iter()
                    .map(|(p, t)| (t.translation, p.0.radius + 4.0)),
            )
    };
    if let Ok((mut probe, mut transform)) = probe.single_mut() {
        let mut velocity = probe.velocity;
        push_out(&mut transform.translation, &mut velocity, bodies());
        probe.velocity = velocity;
    }
    if let Ok((mut core, mut transform)) = cores.single_mut()
        && core.state == CoreState::Free
    {
        let mut velocity = core.velocity;
        push_out(&mut transform.translation, &mut velocity, bodies());
        core.velocity = velocity;
    }
}

#[allow(clippy::type_complexity)]
fn follow(
    piloting: Res<Piloting>,
    view: Res<ProbeView>,
    core_view: Res<CoreView>,
    probe: Query<&Transform, (With<Probe>, Without<MainCamera>, Without<Core>)>,
    cores: Query<&Transform, (With<Core>, Without<MainCamera>, Without<Probe>)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let (Ok(probe), Ok(core), Ok(mut camera)) =
        (probe.single(), cores.single(), camera.single_mut())
    else {
        return;
    };
    let (target, rotation, distance) = match *piloting {
        Piloting::Structure => (probe.translation, view.rotation, view.distance),
        Piloting::Core => (
            core.translation,
            probe.rotation * Quat::from_euler(EulerRot::YXZ, core_view.yaw, core_view.pitch, 0.0),
            core_view.distance,
        ),
    };
    camera.rotation = rotation;
    camera.translation = target + rotation * Vec3::new(0.0, distance * 0.12, distance);
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
