# Architecture

Everything is Rust, in one Cargo workspace:

| Crate | Path | Role |
|---|---|---|
| `mmo-protocol` | `crates/protocol` | Messages between client and server, and the procedural generation both sides share (`procgen.rs`). Depends on neither Bevy nor tokio. |
| `mmo-server` | `crates/server` | The galaxy server, on **tokio**. One process is one galaxy. |
| `mmo-client` | `crates/client` | The game, on **Bevy 0.19**. |

## Protocol (v0.1)

TCP, one JSON object per line (`encode`/`decode` in `mmo-protocol`). JSON was chosen so the messages
stay readable while they change every week; it will move to a binary encoding once they settle.
`PROTOCOL_VERSION` is checked at the handshake.

1. Client → `Hello { protocol, name }`
2. Server → `Welcome { player_id, galaxy, home, spawn_position, spawn_rotation, first_arrival, probes_in_galaxy }` or `Rejected { reason }`
3. Client → `ProbeState { position, rotation }` ten times a second, and `IntroSeen` once the wake-up log is over.
4. Server → `Transfer { address, galaxy_id, ticket }` is reserved for moving a player to another galaxy (not sent yet, see [galaxies.md](galaxies.md)).

In v0.1 the client is trusted for its own movement, and the server only records positions. The
server will take authority over movement once there is something worth cheating for.

## Server

- `main.rs`: options (`--port`, `--galaxy-id`, `--seed`, `--data`), the listener, one tokio task per
  connection, and a saver task (every 5 s when something changed, and on Ctrl+C).
- `galaxy.rs`: the galaxy's state, keyed by probe name: home system index, last position and
  rotation, whether the wake-up log was seen. Saved to `data/<galaxy id>.json` (written to a side
  file, then renamed over the old one). Systems are not saved: the seed regenerates them.

## Client

- `main.rs`: the app, the `Screen` state (`Menu`, `Playing`) and the `Session` resource.
- `net.rs`: the connection on two threads (one reads, one writes) that exchange with the game
  through channels, so no frame ever waits on the network.
- `menu.rs`: the name field (pre-filled at random), the connect button, and the reading of server
  messages on every screen.
- `sky.rs`: the camera (HDR, bloom, no tone mapping) and the star field that follows it.
- `system.rs`: the star, the planets on their orbits, and the star's light (a directional light aimed
  from the star at the probe).
- `probe.rs`: the probe (structure with thrusters, core, payload), the two ways to fly it
  (`Piloting::Structure` with a free view and no fixed up, `Piloting::Core` with the core detached
  and the structure as reference), collisions, and the position reports. The core is its own
  entity, carried by the structure while docked.
- `shapes.rs`: meshes the primitives lack: the drop profile, solids of revolution, tubes along a path.
- `hud.rs`: the fade from black, the wake-up log of a first arrival, readouts, planet labels.

Everything spawned for a session carries `InGame` and is removed when the session ends.
