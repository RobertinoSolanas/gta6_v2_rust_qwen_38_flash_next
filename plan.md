# Plan — "Neon Bay" procedural GTA-style city (pure Rust → WebAssembly)

All code is Rust. No binary assets of any kind: geometry, textures, sky, fonts-free HUD
are generated procedurally at runtime in Rust, rendered with WebGL2 from WASM.

Guiding rules for every step: **small step → `cargo build` → `cargo test` → mark done**.

## Architectural decisions

* **Workspace of small crates = bounded contexts (DDD).** Each crate owns its own
  vocabulary and has no upward dependencies. Pure logic crates compile and test
  natively (`cargo test`), only `city-render` / `city-app` touch the browser.
* **Shared kernel:** `city-math` (vectors, matrices, AABB, hash, deterministic RNG).
  No external math crate → zero-asset, zero-dependency philosophy.
* **Determinism:** one seed → one identical city. Makes native integration tests and
  runtime screenshot tests comparable.
* **Rendering:** single unified vertex format + procedural fragment shading per
  *material id* (asphalt, concrete, facade, grass, glass, metal, skin, paint, emissive).
  Instanced draws for buildings/trees/cars/pedestrians, one shadow map (directional),
  HDR target, cheap bright-pass bloom, ACES tonemap + vignette + grain post.
* **Animation without assets:** humanoid "part palette" (24 bone matrices uploaded once
  per frame) skinned by a `part index` attribute → whole crowd animates in one draw call.
* **Day/night:** `city-sky` owns the sun/moon direction, scattering-ish sky gradient,
  fog colour, exposure, window-light and headlight intensity curves.
* **Testing strategy:** per-crate `tests/` folder (integration-style, separate from code),
  one overall integration crate (`city-integration`), plus *runtime* tests that build the
  real WASM and drive Google Chrome headless (`runtime-tests/run.mjs`) with a handful of
  screenshots + DOM diagnostics.
* **Chrome hygiene (hard rule):** the runtime suite always stops its headless Chrome
  slaves and the static server when it finishes — on success, on test failure, on crash
  and on Ctrl-C (`chrome.mjs` tracks every launched process and reaps the full process
  tree + tmp profile dirs on exit; stale Chrome from a previous crashed run is reaped at
  startup). No headless Chrome may survive a test run, ever.
* **Dependencies (deliberately tiny):** `wasm-bindgen`, `web-sys`, `js-sys`
  (+ `wasm-bindgen-test` for the wasm target). Nothing else.

## Crates, use cases, tests

| # | Crate | Bounded context / use cases | Tests (in `<crate>/tests/`) |
|---|-------|------------------------------|------------------------------|
| 1 | `city-math` | Vectors, Mat4, AABB, angle wrap, smoothstep, hash, PCG RNG, ray/AABB, segment ops | `math_vec.rs`, `math_mat.rs`, `math_geo.rs`, `math_rng.rs` |
| 2 | `city-layout` | City grid generation: blocks/roads/sidewalks/crossings, building lots (height, style, window grid), parks, props (trees, lamps, benches, bins), spatial hash index, walkability + collision resolution, spawn points | `layout_generation.rs`, `layout_index.rs`, `layout_collision.rs` |
| 3 | `city-sky` | Day/night cycle, sun/moon dir, sky gradient params, fog, exposure, star fade, window/headlight curves | `sky_cycle.rs` |
| 4 | `city-sim` | Crowd + traffic: pedestrians walk sidewalks & cross streets, cars drive lanes, turn at intersections, keep spacing, despawn/respawn off-screen | `sim_pedestrians.rs`, `sim_traffic.rs` |
| 5 | `city-avatar` | Third-person character: camera-relative WASD walk/sprint, gravity, ground follow, wall slide, walk-cycle phase, jump | `avatar_controller.rs` |
| 6 | `city-camera` | Orbit/third-person rig, mouse yaw/pitch, pitch clamp, smoothing, occlusion pull-back, look-ahead | `camera_rig.rs` |
| 7 | `city-input` | DOM-free input model: key bindings, action state, mouse delta accumulation, sprint/jump edges, focus-loss reset | `input_model.rs` |
| 8 | `city-tex` | Procedural textures (no images): asphalt, concrete/curb, grass, brick/plaster facade, roof gravel, metal, sidewalk cracks, noise LUT | `tex_generation.rs` |
| 9 | `city-mesh` | Geometry builders: ground/road/sidewalk/crossing mesh, building boxes with UV frames, trees, street lamps, cars, humanoid mesh + part palettes, park props | `mesh_build.rs`, `mesh_rig.rs` |
| 10 | `city-render` | WebGL2 pipeline: shaders (source assembly), VAO/buffer/FBO wrappers, shadow map, bloom, post, frustum culling, adaptive quality | `render_cpu.rs` (shaders, culling, stats) + `wasm_gl.rs` (wasm target) |
| 11 | `city-hud` | HUD model: minimap projection, road/ped/agent primitives, clock/compass string, context tips, radar rotation | `hud_model.rs` |
| 12 | `city-app` | Orchestration: fixed-step loop, world build, wiring input→avatar→camera→sim→render→hud, `#[wasm_bindgen]` API for tests | `app_world.rs` (native headless stepping) |
| 13 | `city-integration` | Whole-app invariants: build → run N ticks → agent/physics/render-data/HUD consistency, determinism, perf budget | `app_smoke.rs`, `determinism.rs`, `perf_budget.rs` |
| 14 | runtime tests | Real browser: build wasm, serve, Chrome headless: boot, walk, night | `runtime-tests/*.mjs` (3–4 screenshots + DOM assertions) |

## Increments

* **I0 — skeleton (done)** workspace, plan, readme, `.gitignore`, empty crates, CI-ish script.
* **I1 — city-math (done)** vectors/mat4/aabb/rng + 4 test files.
* **I2 — city-layout (done)** deterministic grid city, buildings, props, index, collision.
* **I3 — city-sky (done)** day/night model + curves: sun/moon arc (`sun_dir`/`moon_dir`),
  zenith/horizon/glow bands, fog colour + view distance, exposure (`1.0` day → `2.1`
  night), star/window/lamp/headlight/ambient curves, `SkySample` snapshot and
  `SkyClock` (advance / `T` phase-skip / wrap). 30 tests in `city-sky/tests/sky_cycle.rs`.
* **I4 — city-sim (pending)** placeholder crate — pedestrians and traffic are simulated
  nowhere today, so the streets are empty; `city-layout` does generate the lanes, crossings
  and sidewalks they will drive/walk on.
* **I5 — city-avatar + city-camera + city-input (done)** controller, rig, DOM-free input.
  `city-avatar/tests/avatar_controller.rs` now covers it (32 tests): camera-relative
  walk/sprint/back/strafe against the real axis convention (`wish = (strafe, forward)`,
  `right = -fwd.perp()`, camera yaw 0 looks along +X), accel/drag saturation, dt
  clamping, gravity/jump/landing, custom `Terrain` (`update_on`) incl. kerb walk-up and
  wall slide, building collision, walk-cycle phase rate (`speed / stride_len`) and the
  pose invariants. Two controller bugs surfaced while writing them and are fixed in
  `city-avatar/src/lib.rs`: the strafe axis was the **left** axis (`right = fwd.perp()`)
  so `+x` walked to the camera's left, and the sprint gear was applied to the walk-cycle
  length outside the "moving" branch, which made the phase advance with the wrong stride.
* **I6 — city-tex (pending)** placeholder crate; `city-app` currently shades flat palette
  colours per material id instead of generated textures.
* **I7 — city-mesh (pending)** placeholder crate; the geometry that is drawn is built by
  `city-app/src/mesh.rs` (ground, block caps, kerbs, building boxes, parks, props). No
  humanoid rig / part palette exists yet, so the character has no animated mesh.
* **I8 — city-hud (done)** minimap / clock / compass / tips model, painted as a Canvas2D
  vector overlay by `city-app` (no `tests/` folder yet).
* **I9 — city-render (pending)** still the placeholder crate from I0 (~1 line of docs, no
  shader source, no `tests/`). The WebGL2 path that actually runs lives in `city-app`
  (`shaders.rs` = sky + city program, `dom.rs` = buffers/VAO/FBO, one directional light,
  fog, tone map). Shadow map, HDR target, bloom and the rest of the designed pipeline are
  not implemented anywhere yet.
* **I10 — city-app (in progress)** fixed-step world, wasm glue and the page shell run end to
  end; covered by `city-app/tests/app_world.rs`.
* **I11 — city-integration (pending)** the crate is in the workspace but has no `tests/`
  folder yet (cross-crate invariants, determinism and the perf budget are still
  untested).
* **I12 — runtime tests (done)** Chrome headless screenshots `day`, `walk`, `night` plus the
  other checks listed under I15 — rebuilt from scratch in `runtime-tests/` (see I15).
* **I13 — polish & tuning (pending — current phase)** look pass: neon night lighting,
  shopfronts, rooftops, traffic lights, birds/planes?, perf, adaptive quality.
* **I14 — docs & final QA (done)** readme, run instructions, full `cargo test --workspace`.
* **I15 — browser bring-up (done)** the app boots in a browser from a clean tree:
  `city-app` is a `cdylib` (`crate-type = ["cdylib", "rlib"]`), its WebGL2 context asks for
  `preserveDrawingBuffer` (the frame is then readable from JS), `index.html` exposes the
  wasm API as `window.wasm` and boots itself unless the URL carries `?noautoboot`, the HUD
  overlay is `#hud` and stays cleared while hidden, `H` toggles it. `build.sh` / `run.sh` /
  `check.sh` restored, `runtime-tests/` rebuilt with **zero npm dependencies** (CDP over a
  hand-rolled WebSocket): 11 browser checks — boot, content, pixels, walk, sprint, camera,
  night, time skip, HUD, stability, console cleanliness.

## Test status (as measured)

`cargo test --workspace` → **175 passed / 0 failed**; `node runtime-tests/run.mjs` →
**11 / 11** in headless Chrome; `./check.sh` → green.

Per-crate `tests/` folders that exist today: `city-math` (4 files), `city-layout` (3),
`city-sky` (1 — 30 tests), `city-input` (1), `city-avatar` (1 — 32 tests) and `city-app`
(1). Still to write: `city-sim`, `city-camera`, `city-tex`, `city-mesh`, `city-render`,
`city-hud`, `city-integration` — the runtime suite currently covers part of that gap from
the browser side. `city-render` is additionally still a placeholder implementation.

## Definition of done

* `cargo test --workspace` green (native) and `wasm-pack test` compiles.
* `node runtime-tests/run.mjs` → screenshots + all DOM assertions pass, 0 console errors.
* Walkable city: roads, sidewalks, buildings with lit windows, trees, moving cars & peds,
  animated third-person character, day→night cycle, custom-drawn HUD/minimap.
