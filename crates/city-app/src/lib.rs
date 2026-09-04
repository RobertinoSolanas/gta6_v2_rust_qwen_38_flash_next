//! # city-app
//!
//! Orchestration: the fixed-step world (city + clock + avatar + camera + input + HUD) and
//! the `#[wasm_bindgen]` boundary that lets a browser — and the headless Chrome runtime
//! tests — drive it.
//!
//! Layout of this crate:
//! * [`World`] — pure orchestration (no DOM): generate the city, advance the clock, step
//!   the avatar, follow with the camera, produce a [`city_hud::HudFrame`]. It compiles
//!   and is testable natively.
//! * [`app`] — the `#[wasm_bindgen]` boundary: a WebGL2 canvas for the sky/city preview,
//!   a Canvas2D overlay for the HUD, event plumbing and `requestAnimationFrame`.
//!
//! The world logic has no DOM dependency at all, which is what lets `cargo test` cover
//! the exact same code path the browser runs.

#![forbid(unsafe_code)]

pub mod agents;
#[cfg(target_arch = "wasm32")]
pub mod dom;
pub mod mesh;
mod shaders;
pub mod world;

pub use world::{SimSnapshot, World, WorldConfig};

#[cfg(target_arch = "wasm32")]
pub mod wasm;
