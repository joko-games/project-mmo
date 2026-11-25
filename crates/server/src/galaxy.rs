//! The galaxy's state: who has woken up here, which home system is whose, where each probe is.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use mmo_protocol::procgen::{arrival_position, galaxy_info, home_system};
use mmo_protocol::{GalaxyInfo, Quat, Vec3, Welcome};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProbeRecord {
    pub player_id: u64,
    pub home_index: u64,
    pub position: Vec3,
    pub rotation: Quat,
    pub intro_seen: bool,
}

/// What is written to disk. The systems themselves are not: the seed gives them back.
#[derive(Serialize, Deserialize, Debug)]
struct Saved {
    id: String,
    seed: u64,
    next_home: u64,
    probes: BTreeMap<String, ProbeRecord>,
}

pub struct Galaxy {
    pub info: GalaxyInfo,
    path: PathBuf,
    next_home: u64,
    probes: BTreeMap<String, ProbeRecord>,
    dirty: bool,
}

impl Galaxy {
    pub fn load_or_create(data: &Path, id: &str, seed: u64) -> Result<Self> {
        std::fs::create_dir_all(data).with_context(|| format!("creating {}", data.display()))?;
        let path = data.join(format!("{id}.json"));
        if path.exists() {
            let text = std::fs::read_to_string(&path)?;
            let saved: Saved = serde_json::from_str(&text)
                .with_context(|| format!("reading {}", path.display()))?;
            return Ok(Self {
                info: galaxy_info(&saved.id, saved.seed),
                path,
                next_home: saved.next_home,
                probes: saved.probes,
                dirty: false,
            });
        }
        Ok(Self {
            info: galaxy_info(id, seed),
            path,
            next_home: 0,
            probes: BTreeMap::new(),
            dirty: true,
        })
    }

    pub fn probe_count(&self) -> usize {
        self.probes.len()
    }

    /// A probe connects: a known one finds its system and its place again, a new one gets the
    /// next free home system and wakes up at its edge.
    pub fn arrive(&mut self, name: &str) -> Welcome {
        if !self.probes.contains_key(name) {
            let home_index = self.next_home;
            self.next_home += 1;
            let home = home_system(self.info.seed, home_index);
            let record = ProbeRecord {
                player_id: home_index + 1,
                home_index,
                position: arrival_position(&home),
                rotation: [0.0, 0.0, 0.0, 1.0],
                intro_seen: false,
            };
            self.probes.insert(name.to_string(), record);
            self.dirty = true;
        }
        let record = &self.probes[name];
        Welcome {
            player_id: record.player_id,
            galaxy: self.info.clone(),
            home: home_system(self.info.seed, record.home_index),
            spawn_position: record.position,
            spawn_rotation: record.rotation,
            first_arrival: !record.intro_seen,
            probes_in_galaxy: self.probes.len() as u64,
        }
    }

    pub fn moved(&mut self, name: &str, position: Vec3, rotation: Quat) {
        let finite = position.iter().chain(&rotation).all(|v| v.is_finite());
        if let (Some(record), true) = (self.probes.get_mut(name), finite) {
            record.position = position;
            record.rotation = rotation;
            self.dirty = true;
        }
    }

    pub fn intro_seen(&mut self, name: &str) {
        if let Some(record) = self.probes.get_mut(name) {
            record.intro_seen = true;
            self.dirty = true;
        }
    }

    pub fn save_if_dirty(&mut self) -> Result<()> {
        if self.dirty { self.save() } else { Ok(()) }
    }

    pub fn save(&mut self) -> Result<()> {
        let saved = Saved {
            id: self.info.id.clone(),
            seed: self.info.seed,
            next_home: self.next_home,
            probes: self.probes.clone(),
        };
        // Written aside, then moved over the old file: a crash mid-write loses nothing.
        let temporary = self.path.with_extension("json.tmp");
        std::fs::write(&temporary, serde_json::to_string_pretty(&saved)?)?;
        std::fs::rename(&temporary, &self.path)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_probe_gets_its_own_home_and_keeps_it() {
        let dir = std::env::temp_dir().join(format!("mmo-galaxy-test-{}", std::process::id()));
        let mut galaxy = Galaxy::load_or_create(&dir, "test", 9).unwrap();
        let a = galaxy.arrive("Alpha");
        let b = galaxy.arrive("Beta");
        assert_ne!(a.home.index, b.home.index);
        assert!(a.first_arrival);
        galaxy.intro_seen("Alpha");
        galaxy.moved("Alpha", [1.0, 2.0, 3.0], [0.0, 0.0, 0.0, 1.0]);
        galaxy.save().unwrap();

        let mut again = Galaxy::load_or_create(&dir, "test", 9).unwrap();
        let a2 = again.arrive("Alpha");
        assert_eq!(a2.home.index, a.home.index);
        assert_eq!(a2.spawn_position, [1.0, 2.0, 3.0]);
        assert!(!a2.first_arrival);
        std::fs::remove_dir_all(dir).ok();
    }
}
