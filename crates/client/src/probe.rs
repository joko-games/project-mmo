//! The player's probe: its model, how it flies, the camera that follows it, and its position
//! reported to the server.

use std::f32::consts::{FRAC_PI_2, PI};

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
/// How often the probe's position goes to the server.
const REPORT_EVERY: f32 = 0.1;

pub struct ProbePlugin;

impl Plugin for ProbePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Playing), spawn_probe)
            .add_systems(
                Update,
                (orbit_camera, fly, collide, follow, report)
                    .chain()
                    .run_if(in_state(Screen::Playing)),
            );
    }
}

#[derive(Component, Default)]
pub struct Probe {
    pub velocity: Vec3,
    /// 0 to 1: how hard the engines push, for their glow.
    pub thrust: f32,
}

/// The glowing exhausts, scaled with the thrust.
#[derive(Component)]
struct Exhaust;

/// The light on the bus, blinking.
#[derive(Component)]
struct Beacon;

#[derive(Resource)]
pub struct OrbitCamera {
    yaw: f32,
    pitch: f32,
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
    let rotation = Quat::from_array(w.spawn_rotation).normalize();
    // A new probe faces the star; a returning one faces where it was facing.
    let facing = if w.first_arrival {
        Transform::from_translation(position).looking_at(Vec3::ZERO, Vec3::Y)
    } else {
        Transform::from_translation(position).with_rotation(rotation)
    };
    let (yaw, pitch, _) = facing.rotation.to_euler(EulerRot::YXZ);
    commands.insert_resource(OrbitCamera {
        // Three quarters from behind: the star ahead, and the probe seen from the side.
        yaw: yaw + 0.7,
        pitch: pitch - 0.3,
        distance: 14.0,
    });
    commands.insert_resource(ReportTimer(Timer::from_seconds(
        REPORT_EVERY,
        TimerMode::Repeating,
    )));

    let hull = materials.add(StandardMaterial {
        base_color: Color::srgb(0.32, 0.33, 0.36),
        metallic: 0.8,
        perceptual_roughness: 0.35,
        ..default()
    });
    let foil = materials.add(StandardMaterial {
        base_color: Color::srgb(0.85, 0.62, 0.22),
        metallic: 1.0,
        perceptual_roughness: 0.3,
        ..default()
    });
    let white = materials.add(StandardMaterial {
        base_color: Color::srgb(0.92, 0.93, 0.95),
        perceptual_roughness: 0.5,
        emissive: LinearRgba::rgb(0.04, 0.045, 0.06),
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    let dark = materials.add(StandardMaterial {
        base_color: Color::srgb(0.08, 0.08, 0.1),
        perceptual_roughness: 0.6,
        ..default()
    });
    let neon = materials.add(StandardMaterial {
        base_color: Color::srgb(0.3, 0.9, 1.0),
        emissive: LinearRgba::rgb(0.6, 3.0, 4.0),
        ..default()
    });
    let exhaust = materials.add(StandardMaterial {
        base_color: Color::srgb(0.4, 0.7, 1.0),
        emissive: LinearRgba::rgb(1.5, 4.0, 12.0),
        unlit: true,
        ..default()
    });
    let payload = materials.add(StandardMaterial {
        base_color: Color::srgb(0.5, 1.0, 0.6),
        emissive: LinearRgba::rgb(0.6, 2.5, 0.9),
        ..default()
    });
    let beacon = materials.add(StandardMaterial {
        base_color: Color::srgb(1.0, 0.2, 0.15),
        emissive: LinearRgba::rgb(8.0, 0.6, 0.3),
        unlit: true,
        ..default()
    });

    // The probe flies along -Z. Its dish looks back the way it came (+Z), toward a home too far
    // away to see.
    let along_z = Quat::from_rotation_x(FRAC_PI_2);
    let along_x = Quat::from_rotation_z(FRAC_PI_2);
    let mut mesh = |m: Mesh| meshes.add(m);
    let bus = mesh(Cylinder::new(0.95, 0.7).mesh().resolution(10).build());
    let dish = mesh(
        Cone {
            radius: 2.0,
            height: 0.55,
        }
        .mesh()
        .resolution(40)
        .build(),
    );
    let rod = mesh(Cylinder::new(0.035, 1.0).mesh().resolution(6).build());
    let ring = mesh(
        Torus {
            minor_radius: 0.05,
            major_radius: 1.15,
        }
        .mesh()
        .build(),
    );
    let pod = mesh(Cylinder::new(0.16, 0.9).mesh().resolution(12).build());
    let nozzle = mesh(Cylinder::new(0.12, 0.08).mesh().resolution(12).build());
    let rtg = mesh(Cylinder::new(0.2, 0.42).mesh().resolution(8).build());
    let fin = mesh(Cuboid::new(0.6, 0.36, 0.03).mesh().build());
    let orb = mesh(Sphere::new(0.32).mesh().ico(3).expect("fine"));
    let small = mesh(Sphere::new(0.08).mesh().ico(2).expect("fine"));
    let box_mesh = mesh(Cuboid::new(0.25, 0.25, 0.25).mesh().build());

    commands
        .spawn((InGame, Probe::default(), facing, Visibility::default()))
        .with_children(|p| {
            // The bus: a ten-sided body wrapped in gold foil, ringed with light.
            p.spawn((
                Mesh3d(bus),
                MeshMaterial3d(foil.clone()),
                Transform::from_rotation(along_z),
            ));
            p.spawn((
                Mesh3d(ring.clone()),
                MeshMaterial3d(neon.clone()),
                Transform::from_xyz(0.0, 0.0, -0.38).with_rotation(along_z),
            ));
            // The great dish, its apex forward, its bowl toward home, and its feed horn.
            p.spawn((
                Mesh3d(dish),
                MeshMaterial3d(white.clone()),
                Transform::from_xyz(0.0, 0.0, 0.75)
                    .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
            ));
            p.spawn((
                Mesh3d(rod.clone()),
                MeshMaterial3d(hull.clone()),
                Transform::from_xyz(0.0, 0.0, 1.5)
                    .with_rotation(along_z)
                    .with_scale(Vec3::new(2.0, 1.4, 2.0)),
            ));
            p.spawn((
                Mesh3d(small.clone()),
                MeshMaterial3d(neon.clone()),
                Transform::from_xyz(0.0, 0.0, 2.2),
            ));
            // The magnetometer boom, long and thin, out to the right.
            p.spawn((
                Mesh3d(rod.clone()),
                MeshMaterial3d(hull.clone()),
                Transform::from_xyz(3.9, 0.0, 0.0)
                    .with_rotation(along_x)
                    .with_scale(Vec3::new(1.0, 6.0, 1.0)),
            ));
            for x in [3.0, 5.0, 6.9] {
                p.spawn((
                    Mesh3d(box_mesh.clone()),
                    MeshMaterial3d(dark.clone()),
                    Transform::from_xyz(x, 0.0, 0.0),
                ));
            }
            p.spawn((
                Mesh3d(small.clone()),
                MeshMaterial3d(neon.clone()),
                Transform::from_xyz(7.0, 0.0, 0.0),
            ));
            // The power boom to the left: three generators with their fins.
            p.spawn((
                Mesh3d(rod.clone()),
                MeshMaterial3d(hull.clone()),
                Transform::from_xyz(-2.0, 0.0, 0.0)
                    .with_rotation(along_x)
                    .with_scale(Vec3::new(1.5, 2.2, 1.5)),
            ));
            for i in 0..3 {
                let x = -1.9 - i as f32 * 0.5;
                p.spawn((
                    Mesh3d(rtg.clone()),
                    MeshMaterial3d(dark.clone()),
                    Transform::from_xyz(x, 0.0, 0.0).with_rotation(along_x),
                ));
                for k in 0..4 {
                    p.spawn((
                        Mesh3d(fin.clone()),
                        MeshMaterial3d(hull.clone()),
                        Transform::from_xyz(x, 0.0, 0.0)
                            .with_rotation(Quat::from_rotation_x(k as f32 * PI / 4.0))
                            .with_scale(Vec3::new(0.7, 1.0, 1.0)),
                    ));
                }
            }
            // Two engine pods under the bus, their exhausts at the back.
            for x in [-0.75, 0.75] {
                p.spawn((
                    Mesh3d(pod.clone()),
                    MeshMaterial3d(hull.clone()),
                    Transform::from_xyz(x, -0.75, 0.2).with_rotation(along_z),
                ));
                p.spawn((
                    Mesh3d(ring.clone()),
                    MeshMaterial3d(neon.clone()),
                    Transform::from_xyz(x, -0.75, -0.1)
                        .with_rotation(along_z)
                        .with_scale(Vec3::splat(0.16)),
                ));
                p.spawn((
                    Exhaust,
                    Mesh3d(nozzle.clone()),
                    MeshMaterial3d(exhaust.clone()),
                    Transform::from_xyz(x, -0.75, 0.7).with_rotation(along_z),
                ));
            }
            // The payload: the seed of a biosphere, cradled under the bus.
            p.spawn((
                Mesh3d(orb),
                MeshMaterial3d(payload),
                Transform::from_xyz(0.0, -0.55, -0.45),
            ));
            p.spawn((
                Beacon,
                Mesh3d(small),
                MeshMaterial3d(beacon),
                Transform::from_xyz(0.0, 0.42, -0.2),
            ));
        });
}

/// Drag with either mouse button to look around the probe; the wheel brings the camera closer.
fn orbit_camera(
    buttons: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut orbit: ResMut<OrbitCamera>,
) {
    if buttons.pressed(MouseButton::Left) || buttons.pressed(MouseButton::Right) {
        orbit.yaw -= motion.delta.x * 0.005;
        orbit.pitch = (orbit.pitch - motion.delta.y * 0.005).clamp(-1.5, 1.5);
    }
    let notches = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 40.0,
    };
    orbit.distance = (orbit.distance * 0.9f32.powf(notches)).clamp(5.0, 120.0);
}

fn fly(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    orbit: Res<OrbitCamera>,
    mut probe: Query<(&mut Probe, &mut Transform)>,
    mut exhausts: Query<&mut Transform, (With<Exhaust>, Without<Probe>)>,
    mut beacons: Query<&mut Visibility, With<Beacon>>,
) {
    let Ok((mut probe, mut transform)) = probe.single_mut() else {
        return;
    };
    let dt = time.delta_secs();
    let view = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    let forward = view * Vec3::NEG_Z;
    let right = view * Vec3::X;
    let up = view * Vec3::Y;

    let mut input = Vec3::ZERO;
    let pressed = |codes: &[KeyCode]| codes.iter().any(|c| keys.pressed(*c));
    if pressed(&[KeyCode::KeyW, KeyCode::KeyZ, KeyCode::ArrowUp]) {
        input += forward;
    }
    if pressed(&[KeyCode::KeyS, KeyCode::ArrowDown]) {
        input -= forward;
    }
    if pressed(&[KeyCode::KeyD, KeyCode::ArrowRight]) {
        input += right;
    }
    if pressed(&[KeyCode::KeyA, KeyCode::KeyQ, KeyCode::ArrowLeft]) {
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
    probe.thrust = probe.thrust
        + ((input.length() * boost.min(2.0) / 2.0) - probe.thrust) * (dt * 6.0).min(1.0);

    // Under thrust the probe turns to face where it pushes; drifting, it keeps its attitude, so
    // the camera can circle it.
    if input != Vec3::ZERO {
        let target = Transform::default().looking_to(input, up).rotation;
        transform.rotation = transform.rotation.slerp(target, (dt * 2.5).min(1.0));
    }

    for mut exhaust in &mut exhausts {
        let length = 0.2 + probe.thrust * 14.0;
        exhaust.scale = Vec3::new(1.0, length, 1.0);
        exhaust.translation.z = 0.7 + length * 0.04;
    }
    let blink = (time.elapsed_secs() % 1.6) < 0.12;
    for mut visibility in &mut beacons {
        *visibility = if blink {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
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
    orbit: Res<OrbitCamera>,
    probe: Query<&Transform, (With<Probe>, Without<MainCamera>)>,
    mut camera: Query<&mut Transform, With<MainCamera>>,
) {
    let (Ok(probe), Ok(mut camera)) = (probe.single(), camera.single_mut()) else {
        return;
    };
    let view = Quat::from_euler(EulerRot::YXZ, orbit.yaw, orbit.pitch, 0.0);
    camera.rotation = view;
    camera.translation =
        probe.translation + view * Vec3::new(0.0, orbit.distance * 0.12, orbit.distance);
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
