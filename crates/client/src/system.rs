//! The player's home system: its star, its planets on their orbits, and the star's light.

use bevy::mesh::MeshBuilder;
use bevy::mesh::TorusMeshBuilder;
use bevy::prelude::*;
use mmo_protocol::{Planet, PlanetKind};

use crate::probe::Probe;
use crate::{InGame, Screen, Session};

pub struct SystemPlugin;

impl Plugin for SystemPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(Screen::Playing), spawn_system)
            .add_systems(
                Update,
                (move_planets, aim_starlight).run_if(in_state(Screen::Playing)),
            );
    }
}

#[derive(Component)]
pub struct Star {
    pub radius: f32,
}

#[derive(Component)]
pub struct PlanetBody(pub Planet);

#[derive(Component)]
struct Starlight;

fn rgb(c: [f32; 3]) -> Color {
    Color::linear_rgb(c[0], c[1], c[2])
}

pub fn spawn_system(
    mut commands: Commands,
    session: Res<Session>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let system = &session.welcome.home;
    let star_color = rgb(system.star.color);

    // The star: drawn far brighter than white, so the bloom gives it a corona.
    commands
        .spawn((
            InGame,
            Star {
                radius: system.star.radius,
            },
            Mesh3d(meshes.add(Sphere::new(system.star.radius).mesh().ico(5).expect("fine"))),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: star_color,
                emissive: star_color.to_linear() * 30.0,
                unlit: true,
                ..default()
            })),
            Transform::default(),
        ))
        .with_children(|star| {
            // A faint halo around it.
            star.spawn((
                Mesh3d(
                    meshes.add(
                        Sphere::new(system.star.radius * 1.6)
                            .mesh()
                            .ico(4)
                            .expect("fine"),
                    ),
                ),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: star_color.with_alpha(0.08),
                    emissive: star_color.to_linear() * 0.6,
                    unlit: true,
                    alpha_mode: AlphaMode::Add,
                    ..default()
                })),
            ));
        });

    commands.spawn((
        InGame,
        Starlight,
        DirectionalLight {
            color: star_color,
            illuminance: 9_000.0,
            ..default()
        },
        Transform::default(),
    ));

    let orbit_material = materials.add(StandardMaterial {
        base_color: Color::srgba(0.4, 0.6, 1.0, 0.12),
        unlit: true,
        alpha_mode: AlphaMode::Blend,
        ..default()
    });

    for planet in &system.planets {
        let color = rgb(planet.color);
        let mut material = StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.85,
            ..default()
        };
        if planet.kind == PlanetKind::Molten {
            material.emissive = LinearRgba::rgb(1.2, 0.3, 0.05);
        }
        let position = Vec3::from(planet.position_at(0.0));
        commands
            .spawn((
                InGame,
                PlanetBody(planet.clone()),
                Mesh3d(meshes.add(Sphere::new(planet.radius).mesh().ico(5).expect("fine"))),
                MeshMaterial3d(materials.add(material)),
                Transform::from_translation(position),
            ))
            .with_children(|body| {
                if planet.kind == PlanetKind::Viable {
                    // An atmosphere: the first hint that this one might be the one.
                    body.spawn((
                        Mesh3d(
                            meshes.add(
                                Sphere::new(planet.radius * 1.08)
                                    .mesh()
                                    .ico(5)
                                    .expect("fine"),
                            ),
                        ),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: Color::srgba(0.4, 0.75, 1.0, 0.18),
                            emissive: LinearRgba::rgb(0.05, 0.15, 0.3),
                            alpha_mode: AlphaMode::Add,
                            ..default()
                        })),
                    ));
                }
                if planet.has_ring {
                    body.spawn((
                        Mesh3d(
                            meshes.add(
                                TorusMeshBuilder {
                                    torus: Torus {
                                        minor_radius: planet.radius * 0.45,
                                        major_radius: planet.radius * 1.9,
                                    },
                                    minor_resolution: 12,
                                    major_resolution: 96,
                                    ..default()
                                }
                                .build(),
                            ),
                        ),
                        MeshMaterial3d(materials.add(StandardMaterial {
                            base_color: color.mix(&Color::WHITE, 0.4).with_alpha(0.55),
                            alpha_mode: AlphaMode::Blend,
                            perceptual_roughness: 1.0,
                            ..default()
                        })),
                        Transform::from_rotation(Quat::from_rotation_x(0.35))
                            .with_scale(Vec3::new(1.0, 0.03, 1.0)),
                    ));
                }
            });

        // The orbit, traced faintly.
        commands.spawn((
            InGame,
            Mesh3d(
                meshes.add(
                    TorusMeshBuilder {
                        torus: Torus {
                            minor_radius: 0.5,
                            major_radius: planet.orbit_radius,
                        },
                        minor_resolution: 4,
                        major_resolution: 360,
                        ..default()
                    }
                    .build(),
                ),
            ),
            MeshMaterial3d(orbit_material.clone()),
            Transform::default(),
        ));
    }
}

fn move_planets(
    time: Res<Time>,
    session: Res<Session>,
    mut planets: Query<(&PlanetBody, &mut Transform)>,
) {
    let t = time.elapsed_secs_f64() - session.started;
    for (planet, mut transform) in &mut planets {
        transform.translation = Vec3::from(planet.0.position_at(t));
        transform.rotate_y(0.02 * time.delta_secs());
    }
}

/// The star is a point, but a directional light aimed from it at the probe lights everything
/// near the probe as the star would.
fn aim_starlight(
    probe: Query<&Transform, (With<Probe>, Without<Starlight>)>,
    mut light: Query<&mut Transform, With<Starlight>>,
) {
    let (Ok(probe), Ok(mut light)) = (probe.single(), light.single_mut()) else {
        return;
    };
    if probe.translation.length_squared() > 1.0 {
        *light = Transform::default().looking_at(probe.translation, Vec3::Y);
    }
}
