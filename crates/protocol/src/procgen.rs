//! Procedural generation: the same seed gives the same galaxy, on any machine.

use crate::{GalaxyInfo, Planet, PlanetKind, Star, StarSystem};

/// SplitMix64: small, fast, and stable forever, which is what a seed needs.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// In `[0, 1)`.
    pub fn unit(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn range(&mut self, low: f32, high: f32) -> f32 {
        low + (high - low) * self.unit()
    }

    pub fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[(self.next_u64() % items.len() as u64) as usize]
    }
}

fn mix(a: u64, b: u64) -> u64 {
    Rng::new(a ^ b.wrapping_mul(0x9E37_79B9_7F4A_7C15)).next_u64()
}

/// Home systems lie on the galaxy's spiral arms, this many light years apart at the least: far
/// enough that a new probe meets nobody before it is ready to.
pub const HOME_SPACING_LY: f64 = 60.0;

const SYLLABLES: &[&str] = &[
    "ka", "ri", "on", "vel", "tha", "mor", "ae", "lis", "dra", "nox", "sel", "ur", "zan", "ce",
    "pho", "ith", "qua", "rel", "syn", "tor",
];

fn proper_name(rng: &mut Rng, syllables: usize) -> String {
    let mut name: String = (0..syllables).map(|_| *rng.pick(SYLLABLES)).collect();
    if let Some(first) = name.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    name
}

pub fn galaxy_info(id: &str, seed: u64) -> GalaxyInfo {
    let mut rng = Rng::new(seed);
    GalaxyInfo {
        id: id.to_string(),
        name: format!("{} Spiral", proper_name(&mut rng, 3)),
        seed,
    }
}

/// The `index`-th home system of a galaxy: the first ones near the core's edge, the next ones
/// further out along four arms.
pub fn home_system(galaxy_seed: u64, index: u64) -> StarSystem {
    let mut rng = Rng::new(mix(galaxy_seed, index));
    let arm = (index % 4) as f64;
    let along = (index / 4) as f64;
    let radius = 2_000.0 + along * HOME_SPACING_LY * 1.5;
    let angle = arm * std::f64::consts::FRAC_PI_2 + radius / 3_000.0;
    let galactic_position = [
        radius * angle.cos(),
        (rng.unit() as f64 - 0.5) * 40.0,
        radius * angle.sin(),
    ];

    let syllables = 2 + (rng.next_u64() % 2) as usize;
    let name = proper_name(&mut rng, syllables);
    let star_colors: [[f32; 3]; 4] = [
        [1.0, 0.85, 0.6],
        [1.0, 0.7, 0.4],
        [0.85, 0.9, 1.0],
        [1.0, 0.95, 0.85],
    ];
    let star = Star {
        radius: rng.range(40.0, 70.0),
        color: *rng.pick(&star_colors),
    };

    let count = 4 + (rng.next_u64() % 3) as usize;
    let viable = 1 + (rng.next_u64() % 2) as usize;
    let mut planets = Vec::with_capacity(count);
    let mut orbit = star.radius * 4.0;
    for i in 0..count {
        orbit += rng.range(220.0, 420.0);
        let kind = if i == viable {
            PlanetKind::Viable
        } else if i == 0 {
            PlanetKind::Molten
        } else if orbit > 1_500.0 && rng.unit() < 0.6 {
            PlanetKind::GasGiant
        } else if orbit > 1_300.0 {
            PlanetKind::Ice
        } else {
            PlanetKind::Rocky
        };
        let (radius, color) = match kind {
            PlanetKind::Molten => (rng.range(8.0, 14.0), [0.55, 0.18, 0.06]),
            PlanetKind::Rocky => (rng.range(9.0, 18.0), [0.45, 0.38, 0.32]),
            PlanetKind::Viable => (rng.range(14.0, 20.0), [0.16, 0.36, 0.55]),
            PlanetKind::Ice => (rng.range(10.0, 20.0), [0.7, 0.82, 0.9]),
            PlanetKind::GasGiant => (
                rng.range(35.0, 60.0),
                *rng.pick(&[[0.75, 0.55, 0.35], [0.5, 0.6, 0.75], [0.7, 0.65, 0.5]]),
            ),
        };
        planets.push(Planet {
            name: format!("{name} {}", ["I", "II", "III", "IV", "V", "VI", "VII"][i]),
            kind,
            radius,
            orbit_radius: orbit,
            // Slow enough to watch, fast enough to notice: the inner worlds circle in minutes.
            orbit_speed: 0.4 / (orbit / 100.0).powf(1.5),
            orbit_phase: rng.range(0.0, std::f32::consts::TAU),
            color,
            has_ring: kind == PlanetKind::GasGiant && rng.unit() < 0.7,
        });
    }

    StarSystem {
        index,
        name,
        galactic_position,
        star,
        planets,
    }
}

/// Where a probe wakes up in its home system: out past the last orbit, facing the star.
pub fn arrival_position(system: &StarSystem) -> [f32; 3] {
    let outer = system
        .planets
        .iter()
        .map(|p| p.orbit_radius)
        .fold(0.0, f32::max);
    [outer * 0.55, 60.0, outer * 0.55]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_system() {
        assert_eq!(home_system(42, 3), home_system(42, 3));
        assert_ne!(home_system(42, 3), home_system(42, 4));
    }

    #[test]
    fn every_home_has_one_viable_world() {
        for index in 0..50 {
            let system = home_system(7, index);
            let viable = system
                .planets
                .iter()
                .filter(|p| p.kind == PlanetKind::Viable)
                .count();
            assert_eq!(viable, 1, "system {index}");
        }
    }

    #[test]
    fn homes_are_far_apart() {
        let systems: Vec<_> = (0..40)
            .map(|i| home_system(1, i).galactic_position)
            .collect();
        for (i, a) in systems.iter().enumerate() {
            for b in &systems[i + 1..] {
                let d =
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt();
                assert!(d >= HOME_SPACING_LY, "{d}");
            }
        }
    }
}
