//! What the client and the server say to each other, and the procedural generation both sides
//! agree on.
//!
//! The wire format of v0.1 is one JSON object per line over TCP: easy to read in a log, easy to
//! change while the game takes shape. It will move to a binary encoding once the messages settle.

pub mod procgen;

use serde::{Deserialize, Serialize};

/// Bumped whenever a message changes shape; a client and a server of different versions refuse
/// each other at the handshake.
pub const PROTOCOL_VERSION: u32 = 1;

/// The port a galaxy server listens on by default.
pub const DEFAULT_PORT: u16 = 7777;

pub const MIN_NAME_CHARS: usize = 3;
pub const MAX_NAME_CHARS: usize = 24;

/// A name a probe may carry: letters, digits, `-` and `_`, within the length bounds.
pub fn is_valid_name(name: &str) -> bool {
    let count = name.chars().count();
    (MIN_NAME_CHARS..=MAX_NAME_CHARS).contains(&count)
        && name
            .chars()
            .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
}

/// A position in a star system, in game units (one unit is roughly one meter at the probe's
/// scale; distances between systems are in light years and never mixed with these).
pub type Vec3 = [f32; 3];
/// A rotation as a quaternion `[x, y, z, w]`.
pub type Quat = [f32; 4];

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ClientMessage {
    /// The first message of every connection.
    Hello { protocol: u32, name: String },
    /// Where the player's probe is, a few times a second.
    ProbeState { position: Vec3, rotation: Quat },
    /// The player saw the arrival sequence through: it is not played again.
    IntroSeen,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ServerMessage {
    /// The handshake succeeded: who the player is in this galaxy and where they wake up.
    Welcome(Welcome),
    /// The handshake failed; the server closes the connection after it.
    Rejected { reason: String },
    /// Reserved for linking galaxies: the player leaves this server for another one, which knows
    /// them by `ticket`. Not sent yet in v0.1 (see docs/galaxies.md).
    Transfer {
        address: String,
        galaxy_id: String,
        ticket: String,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Welcome {
    pub player_id: u64,
    pub galaxy: GalaxyInfo,
    /// The player's home system: theirs alone, far from anyone else's.
    pub home: StarSystem,
    pub spawn_position: Vec3,
    pub spawn_rotation: Quat,
    /// True until the player has seen the arrival sequence once.
    pub first_arrival: bool,
    /// How many probes this galaxy has woken so far.
    pub probes_in_galaxy: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GalaxyInfo {
    /// Stable across restarts: what other galaxies will use to address this one.
    pub id: String,
    pub name: String,
    pub seed: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StarSystem {
    pub index: u64,
    pub name: String,
    /// Where the system lies in its galaxy, in light years from the galactic center.
    pub galactic_position: [f64; 3],
    pub star: Star,
    pub planets: Vec<Planet>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Star {
    pub radius: f32,
    /// Linear RGB.
    pub color: [f32; 3],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Planet {
    pub name: String,
    pub kind: PlanetKind,
    pub radius: f32,
    pub orbit_radius: f32,
    /// Radians per second along the orbit.
    pub orbit_speed: f32,
    /// Where on its orbit the planet was at the system's time zero.
    pub orbit_phase: f32,
    /// Linear RGB.
    pub color: [f32; 3],
    pub has_ring: bool,
}

impl Planet {
    /// Where the planet is on its orbit at `time` seconds of the system's clock.
    pub fn position_at(&self, time: f64) -> Vec3 {
        let angle = self.orbit_phase as f64 + self.orbit_speed as f64 * time;
        [
            self.orbit_radius * angle.cos() as f32,
            0.0,
            self.orbit_radius * angle.sin() as f32,
        ]
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanetKind {
    Molten,
    Rocky,
    /// The one world of a home system where the payload can take root.
    Viable,
    Ice,
    GasGiant,
}

impl PlanetKind {
    pub fn label(self) -> &'static str {
        match self {
            PlanetKind::Molten => "molten",
            PlanetKind::Rocky => "rocky",
            PlanetKind::Viable => "viable?",
            PlanetKind::Ice => "ice",
            PlanetKind::GasGiant => "gas giant",
        }
    }
}

/// One message as a line of the wire format.
pub fn encode<T: Serialize>(message: &T) -> String {
    let mut line = serde_json::to_string(message).expect("messages always serialize");
    line.push('\n');
    line
}

pub fn decode<'a, T: Deserialize<'a>>(line: &'a str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_valid_name("Probe-7_Kestrel"));
        assert!(!is_valid_name("ab"));
        assert!(!is_valid_name("has space"));
        assert!(!is_valid_name(&"x".repeat(MAX_NAME_CHARS + 1)));
    }

    #[test]
    fn round_trip() {
        let message = ClientMessage::Hello {
            protocol: PROTOCOL_VERSION,
            name: "Probe".into(),
        };
        let line = encode(&message);
        assert!(line.ends_with('\n'));
        assert_eq!(decode::<ClientMessage>(&line).unwrap(), message);
    }
}
