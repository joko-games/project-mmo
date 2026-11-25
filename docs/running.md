# Building and running

Requires Rust (edition 2024). On Linux, Bevy also needs the system packages listed in the CI
workflow.

```powershell
./run-release.ps1
```

This builds the workspace in release, starts a galaxy server on port 7777 if none is listening
(its output goes to `target/server.log`), and opens the game. Closing the game stops the server
it started. `-SkipBuild` plays the existing build; `-Clients 2` opens two windows on the same
galaxy.

By hand:

```powershell
cargo run --release -p mmo-server -- --port 7777 --galaxy-id galaxy-0 --seed 1234
cargo run --release -p mmo-client -- --server 127.0.0.1:7777
```

The galaxy is saved in `data/<galaxy id>.json`. Delete that file to start the galaxy over.

## Controls

| Input | Action |
|---|---|
| Z, S, Q, D on AZERTY (W, S, A, D on QWERTY), or arrows | thrust forward, back, left, right (relative to the view) |
| A / E on AZERTY (Q / E on QWERTY) | roll the view |
| Space / Ctrl or C | rise / sink, relative to the view |
| Shift | long-range drive (6× thrust and top speed) |
| X | brake |
| Mouse drag (either button) | turn the view, on its own axes: there is no fixed up |
| Wheel | camera distance |
| Tab | detach the core and fly it (simpler navigation, no roll, no inertia); Tab again to send it back |

## Tests and checks

```powershell
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The same three run in CI on every push to `main` (`.github/workflows/ci.yml`).

## Testing without a keyboard

- `mmo-client --connect` presses WAKE UP at once, with the random name.
- `mmo-client --capture shot.png` connects, waits 9 seconds in the game, saves a screenshot and
  quits. That is how a change can be checked by eye without anyone at the controls.
- `--in-core` with `--capture`: the core is detached two seconds in, and the picture is the core's
  view.
