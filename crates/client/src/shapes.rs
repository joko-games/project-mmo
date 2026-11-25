//! Meshes the primitives do not give: a tube along a path, and a solid of revolution.

use bevy::asset::RenderAssetUsages;
use bevy::mesh::{Indices, PrimitiveTopology};
use bevy::prelude::*;

/// The probe's silhouette, along its axis: a drop pulled backwards. Round at the nose (toward -Z),
/// widest at `0`, tapering to a point at the tail (toward +Z).
pub struct Drop {
    /// Where the nose is, ahead of the widest point (a positive length).
    pub nose: f32,
    /// How far behind the widest point the tail ends.
    pub tail: f32,
    pub radius: f32,
    /// The radius left at the very end of the tail.
    pub tail_radius: f32,
}

impl Drop {
    /// The radius at `z` along the axis.
    pub fn radius_at(&self, z: f32) -> f32 {
        if z <= 0.0 {
            let u = (z / self.nose).clamp(-1.0, 0.0);
            self.radius * (1.0 - u * u).max(0.0).sqrt()
        } else {
            let s = (z / self.tail).clamp(0.0, 1.0);
            // cos(pi/2) comes out a hair below zero in f32, and a negative number to a fractional
            // power is NaN: one NaN vertex and the whole mesh is culled.
            let taper = (s * std::f32::consts::FRAC_PI_2).cos().max(0.0).powf(1.4);
            self.tail_radius + (self.radius - self.tail_radius) * taper
        }
    }

    /// `(z, radius)` from the nose to the tail, `steps + 1` of them, denser at the round nose.
    pub fn profile(&self, steps: usize) -> Vec<(f32, f32)> {
        (0..=steps)
            .map(|i| {
                let t = i as f32 / steps as f32;
                let front = 0.35;
                let z = if t < front {
                    // Nose: even steps in angle, so the round part stays round.
                    let a = (t / front) * std::f32::consts::FRAC_PI_2;
                    -self.nose * a.cos()
                } else {
                    self.tail * (t - front) / (1.0 - front)
                };
                (z, self.radius_at(z))
            })
            .collect()
    }
}

/// A closed-ish solid of revolution around the Z axis, from `(z, radius)` pairs in order of z.
pub fn lathe(profile: &[(f32, f32)], sides: usize) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let rings = profile.len();
    for (i, &(z, r)) in profile.iter().enumerate() {
        let before = profile[i.saturating_sub(1)];
        let after = profile[(i + 1).min(rings - 1)];
        let (dz, dr) = (after.0 - before.0, after.1 - before.1);
        for j in 0..=sides {
            let theta = j as f32 / sides as f32 * std::f32::consts::TAU;
            let (sin, cos) = theta.sin_cos();
            positions.push([r * cos, r * sin, z]);
            normals.push(
                Vec3::new(dz * cos, dz * sin, -dr)
                    .normalize_or(Vec3::Z)
                    .to_array(),
            );
            uvs.push([j as f32 / sides as f32, i as f32 / (rings - 1) as f32]);
        }
    }
    build(positions, normals, uvs, grid_indices(rings, sides))
}

/// A tube of `radius` following `path`, open at both ends.
pub fn tube(path: &[Vec3], radius: f32, sides: usize) -> Mesh {
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = Vec::new();
    let n = path.len();
    let tangent = |k: usize| (path[(k + 1).min(n - 1)] - path[k.saturating_sub(1)]).normalize();
    // A frame carried along the path without twisting.
    let first = tangent(0);
    let mut normal = first.any_orthonormal_vector();
    for (k, point) in path.iter().enumerate() {
        let t = tangent(k);
        normal = (normal - t * normal.dot(t)).normalize_or(t.any_orthonormal_vector());
        let binormal = t.cross(normal);
        for j in 0..=sides {
            let phi = j as f32 / sides as f32 * std::f32::consts::TAU;
            let out = normal * phi.cos() + binormal * phi.sin();
            positions.push((*point + out * radius).to_array());
            normals.push(out.to_array());
            uvs.push([j as f32 / sides as f32, k as f32 / (n - 1) as f32]);
        }
    }
    build(positions, normals, uvs, grid_indices(n, sides))
}

/// Two triangles per quad of a grid of `rings` rings of `sides + 1` vertices, facing outward
/// when the rings advance along the axis and the vertices turn counterclockwise around it.
fn grid_indices(rings: usize, sides: usize) -> Vec<u32> {
    let mut indices = Vec::new();
    let row = (sides + 1) as u32;
    for i in 0..rings.saturating_sub(1) as u32 {
        for j in 0..sides as u32 {
            let a = i * row + j;
            let b = a + 1;
            let c = a + row;
            let d = c + 1;
            indices.extend_from_slice(&[a, b, c, b, d, c]);
        }
    }
    indices
}

fn build(
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    indices: Vec<u32>,
) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uvs)
    .with_inserted_indices(Indices::U32(indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DROP: Drop = Drop {
        nose: 1.5,
        tail: 5.0,
        radius: 1.6,
        tail_radius: 0.2,
    };

    #[test]
    fn a_drop_is_widest_at_zero_and_longer_behind() {
        assert!((DROP.radius_at(0.0) - 1.6).abs() < 1e-5);
        assert!(DROP.radius_at(-1.5) < 1e-3);
        assert!((DROP.radius_at(5.0) - 0.2).abs() < 1e-5);
        assert!(DROP.radius_at(2.0) > DROP.radius_at(4.0));
        for i in 0..=1000 {
            let z = -1.5 + 6.5 * i as f32 / 1000.0;
            assert!(DROP.radius_at(z).is_finite(), "{z}");
        }
    }

    #[test]
    fn meshes_have_matching_attributes() {
        let profile = DROP.profile(16);
        assert_eq!(profile.first().unwrap().0, -1.5);
        let mesh = lathe(&profile, 12);
        assert_eq!(mesh.count_vertices(), 17 * 13);
        let path: Vec<Vec3> = (0..5)
            .map(|i| Vec3::new(0.0, i as f32, i as f32 * 0.5))
            .collect();
        assert_eq!(tube(&path, 0.1, 6).count_vertices(), 5 * 7);
    }
}
