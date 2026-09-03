# Neon Bay — procedural GTA-style city (Rust → WebAssembly, zero assets)

A walkable city that is generated from a single seed at runtime: road grid, sidewalks,
crossings, blocks with buildings, parks, trees and street lamps — walked through by a
third-person character (WASD / mouse look / Shift sprint) under a full day/night cycle
whose lamps, exposure and sky curves follow the clock.

What is **not** there yet (open increments in [plan.md](plan.md)): traffic and pedestrian
simulation (`city-sim`), procedural textures (`city-tex`), the animated humanoid rig
(`city-mesh`) and the full shadow/HDR/bloom pipeline (`city-render`) are placeholder
crates — the streets are empty and the character is rendered as the camera focus, not yet
as an animated body.

Everything is **code**: geometry, sky, lighting and the HUD are produced by Rust at
runtime. There are no image, model, font or audio files in this repository.

## Build & run

```sh
# 1. build the wasm package (writes web/pkg/)
./build.sh                    # add --release for the fast build

# 2. serve the web/ folder and open http://127.0.0.1:8080/
./run.sh            # or: ./run.sh 9000 · python3 -m http.server 8080 --directory web
```

`build.sh` / `run.sh` / `check.sh` put `$HOME/.cargo/bin` on `PATH` themselves, and
`run.sh` refuses to start when `web/pkg/` is missing.

Controls: **W A S D** walk · **Shift** sprint · **mouse** look (click canvas to grab the
pointer) · **Space** jump · **F** cycle camera distance · **T** skip to the next phase of
the day · **H** hide/show the HUD.

Requirements: Rust ≥ 1.85 (`wasm32-unknown-unknown` target), `wasm-pack`, any WebGL2
browser. Chrome headless (`$CHROME_BIN`, defaults to `google-chrome`) is used for the
runtime tests; nothing is installed by them.

## Testing

```sh
cargo test --workspace                      # all unit + integration tests (native)
cargo test -p city-layout                   # one bounded context
./build.sh && node runtime-tests/run.mjs    # real Chrome: boot / walk / night screenshots
./check.sh                                  # build + all native tests + wasm build
```

The runtime suite has **no npm dependencies**: `runtime-tests/chrome.mjs` talks the
Chrome DevTools Protocol directly (raw WebSocket framing) and launches Chrome with
SwiftShader, so WebGL2 works without a GPU. It boots the page with `?noautoboot`
so the WebGL2 context keeps the app's own attributes (`preserveDrawingBuffer`),
which is what lets the suite read rendered pixels back. Artifacts (screenshots +
`report.json`) go to `runtime-tests/artifacts/`.

**Cleanup rule: the suite must never leave headless Chrome slaves running.**
`chrome.mjs` registers every Chrome it launches and reaps them (process tree +
tmp profile dir) on `stop()`, on `SIGINT`/`SIGTERM` and on process exit; it also
reaps Chrome instances leaked by a *previous* crashed run before starting, and
`Browser.close` is requested before killing the process. When driving Chrome by
hand (outside the suite), always kill it when you are done:
`pkill -f 'user-data-dir=/tmp/neonbay-chrome-'`.

Tests live in each crate's own `tests/` folder, separate from the source. Today that is
`city-math` (4 files), `city-layout` (3), `city-sky` (1), `city-input` (1) and `city-app` (1);
`city-sim`, `city-avatar`, `city-camera`, `city-tex`, `city-mesh`, `city-render`,
`city-hud` and `city-integration` still need theirs (see *Test status* in
[plan.md](plan.md)), which the browser suite currently covers in part.

## Architecture (bounded contexts)

| crate | responsibility |
|---|---|
| `city-math` | shared kernel: vec/mat/AABB/hash/PCG-RNG (no external math crate) |
| `city-layout` | city generation: blocks, roads, sidewalks, lots, props, spatial index, collision |
| `city-sky` | day/night: sun & moon direction, sky gradient, fog, exposure, light curves |
| `city-sim` | *placeholder* — traffic lanes + car flow, pedestrian crowd steering |
| `city-avatar` | third-person character controller and skeletal pose |
| `city-camera` | orbit rig, mouse look, occlusion pull-back, smoothing |
| `city-input` | DOM-independent keyboard/mouse action model |
| `city-tex` | *placeholder* — procedural material textures (asphalt, concrete, grass, brick, metal, …) |
| `city-mesh` | *placeholder* — geometry builders incl. the humanoid rig (what is drawn today comes from `city-app/src/mesh.rs`) |
| `city-render` | *placeholder* — WebGL2 renderer: shadow map, HDR, bloom, culling (the live GL path is in `city-app`) |
| `city-hud` | minimap / clock / compass / tips model (drawn as vector overlay) |
| `city-app` | fixed-step world orchestration + `#[wasm_bindgen]` boundary |
| `city-integration` | *placeholder* — whole-app invariants, determinism, performance budget |

Dependencies are intentionally minimal: `wasm-bindgen`, `web-sys`, `js-sys`
(+ `wasm-bindgen-test`). The pure-logic crates compile natively, which is what makes the
large test suite possible without a browser.

## Progress

Implementation state, crate-by-crate use cases and the measured test status live in
**[plan.md](plan.md)**. Current phase: **I13 — polish & tuning** — the app boots and runs
in the browser (**I15** done, 143 native + 11 runtime tests green); what is left is the
visual pass and the missing per-crate test folders.
