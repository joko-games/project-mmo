# Project MMO

A "Minecraft in space": a block-building space sandbox inspired by Avorion and StarMade,
multiplayer, with one server per galaxy. Rust everywhere it fits: a Bevy 0.19 client, a tokio
server and a shared protocol crate. The details live in `docs/` (start at `docs/README.md`).

## Standing decisions (from the user)

- 2026-11-25: The player starts as a probe (a futuristic Voyager), sent from very far away, which
  wakes up in a home system of its own to plant its life payload on a viable world. That system is
  the tutorial and the base. The distance to other players is what protects new players.
- 2026-11-25: The goal is a "false goal". Delivering the payload (about 10 h) is the official end,
  but it is optional. The real game is the sandbox: ship building as in Avorion, and dynamic blocks
  as in StarMade (push, pull, signals, logic).
- 2026-11-25: The probe is not realistic. It is three concentric parts: outside, the frame (structure
  and thrusters); inside it, the thinking core, which will detach in some situations; at the
  core's center, the payload.
- 2026-11-25: The player must never be able to tell where up is, except in the building mode to come.
- 2026-11-25: Whatever can sensibly be written in Rust is written in Rust.
- 2026-11-25: One server instance is one galaxy. Galaxies will be linked, so a player must be able
  to move from one server to another.
- 2026-11-25: The game must be playable with `./run-release.ps1` (it builds, starts a local server
  and opens the game).

## Status

v0.1.0: a menu with a random name and a connect button; a local server; the probe wakes up (fade
from black, and the wake-up log on the first arrival); flight in a procedural home system; the
server saves the probe's position. v0.1.1: the probe redrawn as frame, core and payload; a free
view with roll and no fixed up.

Next: not decided yet. Blocks, mining and the payload are all open.
