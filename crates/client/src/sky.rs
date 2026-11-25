//! The camera and what lies behind everything: a field of distant stars and the band of the
//! galaxy's disc, which travel with the camera so they never get closer.

use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::prelude::*;
use mmo_protocol::procgen::Rng;

use crate::Screen;

/// How far the stars are drawn: beyond every system, within the camera's far plane.
const SKY_RADIUS: f32 = 40_000.0;
pub const FAR_PLANE: f32 = 60_000.0;

pub struct SkyPlugin;

impl Plugin for SkyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(GlobalAmbientLight {
            color: Color::srgb(0.6, 0.7, 1.0),
            brightness: 160.0,
            ..default()
        })
        .add_systems(Startup, (spawn_camera, spawn_stars))
        .add_systems(Update, drift_in_menu.run_if(in_state(Screen::Menu)))
        .add_systems(
            PostUpdate,
            keep_sky_around_camera.before(TransformSystems::Propagate),
        );
    }
}

#[derive(Component)]
pub struct MainCamera;

#[derive(Component)]
struct Sky;

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        MainCamera,
        Camera3d::default(),
        Tonemapping::None,
        Bloom {
            intensity: 0.25,
            ..Bloom::NATURAL
        },
        Projection::Perspective(PerspectiveProjection {
            fov: 60f32.to_radians(),
            near: 0.1,
            far: FAR_PLANE,
            ..default()
        }),
        Transform::from_xyz(0.0, 0.0, 0.0),
    ));
}

fn spawn_stars(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let mesh = meshes.add(Sphere::new(1.0).mesh().ico(1).expect("small subdivision"));
    let tints = [
        Color::srgb(1.0, 1.0, 1.0),
        Color::srgb(0.75, 0.85, 1.0),
        Color::srgb(1.0, 0.85, 0.65),
        Color::srgb(1.0, 0.7, 0.6),
        Color::srgb(0.7, 0.8, 1.0),
    ];
    let materials: Vec<_> = tints
        .iter()
        .flat_map(|tint| {
            [0.6, 1.5, 4.0].map(|glow| {
                materials.add(StandardMaterial {
                    base_color: *tint,
                    emissive: tint.to_linear() * glow,
                    unlit: true,
                    ..default()
                })
            })
        })
        .collect();

    let mut rng = Rng::new(0xC0FFEE);
    // The galactic disc tilted across the sky.
    let band = Quat::from_rotation_x(0.45) * Quat::from_rotation_z(0.2);
    let mut sky = commands.spawn((Sky, Transform::default(), Visibility::default()));
    sky.with_children(|parent| {
        for i in 0..2600 {
            let direction = if i % 5 < 3 {
                // In the band: mostly near the disc's plane.
                let angle = rng.range(0.0, std::f32::consts::TAU);
                let spread = (rng.unit() - 0.5) * (rng.unit() * 0.35);
                band * Vec3::new(angle.cos(), spread, angle.sin()).normalize()
            } else {
                let z = rng.range(-1.0, 1.0);
                let angle = rng.range(0.0, std::f32::consts::TAU);
                let r = (1.0 - z * z).sqrt();
                Vec3::new(r * angle.cos(), z, r * angle.sin())
            };
            let brightness = rng.unit().powi(3);
            let tier = if brightness > 0.6 {
                2
            } else if brightness > 0.2 {
                1
            } else {
                0
            };
            let tint = (rng.next_u64() % tints.len() as u64) as usize;
            let size = 14.0 + brightness * 40.0;
            parent.spawn((
                Mesh3d(mesh.clone()),
                MeshMaterial3d(materials[tint * 3 + tier].clone()),
                Transform::from_translation(direction * SKY_RADIUS).with_scale(Vec3::splat(size)),
            ));
        }
    });
}

fn keep_sky_around_camera(
    camera: Query<&Transform, (With<MainCamera>, Without<Sky>)>,
    mut sky: Query<&mut Transform, With<Sky>>,
) {
    let (Ok(camera), Ok(mut sky)) = (camera.single(), sky.single_mut()) else {
        return;
    };
    sky.translation = camera.translation;
}

/// Behind the menu, the camera turns slowly among the stars.
fn drift_in_menu(time: Res<Time>, mut camera: Query<&mut Transform, With<MainCamera>>) {
    let Ok(mut camera) = camera.single_mut() else {
        return;
    };
    let t = time.elapsed_secs() * 0.02;
    camera.translation = Vec3::ZERO;
    camera.rotation = Quat::from_rotation_y(t) * Quat::from_rotation_x(0.35);
}
