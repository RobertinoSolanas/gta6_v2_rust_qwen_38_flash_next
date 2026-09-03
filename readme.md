# Neon Bay — procedural GTA-style city (Rust → WebAssembly, zero assets)

A walkable city that is generated from a single seed at runtime: road grid, sidewalks,
crossings, blocks with buildings (procedural facades + lit windows), parks, trees,
street lamps, driving cars and walking pedestrians — plus an animated third-person
character (WASD / mouse look / Shift sprint) and a full day/night cycle.

Everything is **code**: meshes, textures, sky, lighting and the HUD are produced by Rust
at runtime. There are no image, model, font or audio files in this repository.

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
pointer) · **Space** jump · **F** cycle camera distance · **T** time skip · **/** hide HUD.

Requirements: Rust ≥ 1.85 (`wasm32-unknown-unknown` target), `wasm-pack`, any WebGL2
browser. Chrome headless is used for the runtime tests.

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

Every crate has its own `tests/` folder with tests kept separate from the source.
`crates/city-integration` holds the cross-crate app tests; `runtime-tests/` drives
Google Chrome headless and writes screenshots to `runtime-tests/artifacts/`.

## Architecture (bounded contexts)

| crate | responsibility |
|---|---|
| `city-math` | shared kernel: vec/mat/AABB/hash/PCG-RNG (no external math crate) |
| `city-layout` | city generation: blocks, roads, sidewalks, lots, props, spatial index, collision |
| `city-sky` | day/night: sun & moon direction, sky gradient, fog, exposure, light curves |
| `city-sim` | traffic lanes + car flow, pedestrian crowd steering |
| `city-avatar` | third-person character controller and skeletal pose |
| `city-camera` | orbit rig, mouse look, occlusion pull-back, smoothing |
| `city-input` | DOM-independent keyboard/mouse action model |
| `city-tex` | procedural material textures (asphalt, concrete, grass, brick, metal, …) |
| `city-mesh` | geometry builders: ground/roads, buildings, trees, lamps, cars, humanoid rig |
| `city-render` | WebGL2 renderer: shadow map, sky, HDR, bloom, tonemap post, culling |
| `city-hud` | minimap / clock / compass / tips model (drawn as vector overlay) |
| `city-app` | fixed-step world orchestration + `#[wasm_bindgen]` boundary |
| `city-integration` | whole-app invariants, determinism, performance budget |

Dependencies are intentionally minimal: `wasm-bindgen`, `web-sys`, `js-sys`
(+ `wasm-bindgen-test`). The pure-logic crates compile natively, which is what makes the
large test suite possible without a browser.

## Progress

Implementation state, crate-by-crate use cases and the test matrix live in
**[plan.md](plan.md)**. Current phase: **I15 — browser bring-up** (build, native tests and the headless
Chrome runtime suite all pass from a clean build).
