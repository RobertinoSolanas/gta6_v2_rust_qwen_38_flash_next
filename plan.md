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
* **I3 — city-sky (done)** day/night model + curves.
* **I4 — city-sim (done)** pedestrians + cars on the generated street graph.
* **I5 — city-avatar + city-camera + city-input (done)** controller, rig, DOM-free input.
* **I6 — city-tex (done)** procedural material textures.
* **I7 — city-mesh (done)** all geometry + rigs (CPU side).
* **I8 — city-hud (done)** minimap/clock/tips model.
* **I9 — city-render (done)** WebGL2: shadow, sky, HDR, bloom, post.
* **I10 — city-app (done)** wasm glue, fixed step loop, page shell, autotest hooks.
* **I11 — city-integration (done)** cross-crate invariants + perf budget.
* **I12 — runtime tests (done)** Chrome headless screenshots: `day`, `walk/sprint`, `night`.
* **I13 — polish & tuning (in progress)** look pass: neon night lighting, shopfronts,
  rooftops, traffic lights, birds/planes?, perf, adaptive quality.
* **I14 — docs & final QA (done)** readme, run instructions, full `cargo test --workspace`,
  clippy/fmt clean-ish, runtime suite green.

## Definition of done

* `cargo test --workspace` green (native) and `wasm-pack test` compiles.
* `node runtime-tests/run.mjs` → 3 screenshots + all DOM assertions pass, 0 console errors.
* Walkable city: roads, sidewalks, buildings with lit windows, trees, moving cars & peds,
  animated third-person character, day→night cycle, custom-drawn HUD/minimap.
