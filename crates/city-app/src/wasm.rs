//! `#[wasm_bindgen]` boundary: boot, DOM events, rAF loop, diagnostics for the tests.

use crate::dom::{start, App};
use std::cell::RefCell;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AddEventListenerOptions, HtmlCanvasElement, WheelEvent};

thread_local! {
    /// The one running app.
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
    /// The self re-registering animation-frame callback.
    static LOOP_CB: RefCell<Option<Closure<dyn FnMut(f64)>>> = const { RefCell::new(None) };
}

/// Boot: build the renderer, install the event handlers, start the loop.
#[wasm_bindgen]
pub fn boot() -> Result<(), JsValue> {
    log("neon-bay: booting");
    let app = start().map_err(|e| {
        log(&format!("neon-bay: {e}"));
        JsValue::from_str(&e)
    })?;
    APP.with(|slot| *slot.borrow_mut() = Some(app));
    install_events();
    start_loop();
    log("neon-bay: ready");
    Ok(())
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Run `f` on the live app (no-op before boot).
fn with_app<T: Default, F: FnOnce(&mut App) -> T>(f: F) -> T {
    APP.with(|slot| match slot.borrow_mut().as_mut() {
        Some(app) => f(app),
        None => T::default(),
    })
}

fn window() -> web_sys::Window {
    web_sys::window().expect("no window")
}

fn log(msg: &str) {
    web_sys::console::log_1(&JsValue::from_str(msg));
}

/// Keys the game consumes (so the page itself does not scroll).
fn is_game_key(key: &str) -> bool {
    matches!(
        key,
        "w" | "a"
            | "s"
            | "d"
            | "W"
            | "A"
            | "S"
            | "D"
            | " "
            | "ArrowUp"
            | "ArrowDown"
            | "ArrowLeft"
            | "ArrowRight"
            | "f"
            | "F"
            | "t"
            | "T"
            | "h"
            | "H"
    )
}

fn city_canvas() -> HtmlCanvasElement {
    window()
        .document()
        .and_then(|d| d.get_element_by_id("city"))
        .and_then(|e| e.dyn_into::<HtmlCanvasElement>().ok())
        .expect("missing <canvas id=\"city\">")
}

fn pointer_locked() -> bool {
    window()
        .document()
        .and_then(|d| d.pointer_lock_element())
        .is_some()
}

// ---------------------------------------------------------------------------
// events + loop
// ---------------------------------------------------------------------------

fn install_events() {
    let win = window();
    let canvas = city_canvas();
    let doc = win.document().expect("no document");

    // ---- keyboard ------------------------------------------------------
    let kb_down = Closure::wrap(Box::new(|e: web_sys::KeyboardEvent| {
        let key = e.key();
        if is_game_key(&key) {
            e.prevent_default();
        }
        with_app(|app| app.world_mut().key(&key, true));
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let kb_up = Closure::wrap(Box::new(|e: web_sys::KeyboardEvent| {
        let key = e.key();
        with_app(|app| app.world_mut().key(&key, false));
    }) as Box<dyn FnMut(web_sys::KeyboardEvent)>);
    let targets: [&web_sys::EventTarget; 3] = [doc.as_ref(), canvas.as_ref(), win.as_ref()];
    for target in targets.into_iter() {
        let _ =
            target.add_event_listener_with_callback("keydown", kb_down.as_ref().unchecked_ref());
        let _ = target.add_event_listener_with_callback("keyup", kb_up.as_ref().unchecked_ref());
    }

    // ---- mouse look ----------------------------------------------------
    let mv = Closure::wrap(Box::new(|e: web_sys::MouseEvent| {
        with_app(|app| {
            if app.world().input().pointer_locked {
                app.world_mut().mouse(e.movement_x() as f32, e.movement_y() as f32);
            }
        });
    }) as Box<dyn FnMut(web_sys::MouseEvent)>);
    let _ = win.add_event_listener_with_callback("mousemove", mv.as_ref().unchecked_ref());

    // ---- click captures the pointer ------------------------------------
    let click = Closure::wrap(Box::new(|_e: web_sys::MouseEvent| {
        if !pointer_locked() {
            let _ = city_canvas().request_pointer_lock();
        }
    }) as Box<dyn FnMut(web_sys::MouseEvent)>);
    let _ = canvas.add_event_listener_with_callback("mousedown", click.as_ref().unchecked_ref());

    // ---- pointer-lock state -------------------------------------------
    let lock = Closure::wrap(Box::new(|_e: web_sys::Event| {
        let locked = pointer_locked();
        with_app(|app| app.world_mut().set_pointer_locked(locked));
    }) as Box<dyn FnMut(web_sys::Event)>);
    let _ = doc.add_event_listener_with_callback(
        "pointerlockchange",
        lock.as_ref().unchecked_ref(),
    );

    // ---- wheel ---------------------------------------------------------
    let wh = Closure::wrap(Box::new(|e: WheelEvent| {
        e.prevent_default();
        let dy = e.delta_y() as f32;
        let ticks = if e.delta_mode() == WheelEvent::DOM_DELTA_LINE {
            dy
        } else {
            dy / 60.0
        };
        with_app(|app| app.world_mut().wheel(ticks.clamp(-2.0, 2.0)));
    }) as Box<dyn FnMut(WheelEvent)>);
    let opts = AddEventListenerOptions::new();
    opts.set_passive(false);
    let _ = canvas.add_event_listener_with_callback_and_add_event_listener_options(
        "wheel",
        wh.as_ref().unchecked_ref(),
        &opts,
    );

    // ---- focus loss ----------------------------------------------------
    let blur = Closure::wrap(Box::new(|_e: web_sys::Event| {
        with_app(|app| app.world_mut().set_pointer_locked(false));
    }) as Box<dyn FnMut(web_sys::Event)>);
    let _ = win.add_event_listener_with_callback("blur", blur.as_ref().unchecked_ref());

    // ---- resize --------------------------------------------------------
    let resize = Closure::wrap(Box::new(|_e: web_sys::Event| {
        with_app(|app| app.resize());
    }) as Box<dyn FnMut(web_sys::Event)>);
    let _ = win.add_event_listener_with_callback("resize", resize.as_ref().unchecked_ref());

    // they must outlive this function
    std::mem::forget(kb_down);
    std::mem::forget(kb_up);
    std::mem::forget(mv);
    std::mem::forget(click);
    std::mem::forget(lock);
    std::mem::forget(wh);
    std::mem::forget(blur);
    std::mem::forget(resize);
}

/// Draw one frame and ask for the next one.
fn pump() {
    with_app(|app| app.frame(js_sys::Date::now()));
    let f = LOOP_CB.with(|c| {
        c.borrow()
            .as_ref()
            .map(|c| c.as_ref().unchecked_ref::<js_sys::Function>().clone())
    });
    if let Some(f) = f {
        let _ = window().request_animation_frame(&f);
    }
}

fn start_loop() {
    let cb: Closure<dyn FnMut(f64)> = Closure::wrap(Box::new(move |_now: f64| pump()));
    let handle = cb.as_ref().unchecked_ref::<js_sys::Function>().clone();
    LOOP_CB.with(|slot| *slot.borrow_mut() = Some(cb));
    let _ = window().request_animation_frame(&handle);
}

// ---------------------------------------------------------------------------
// JS API (page + runtime tests)
// ---------------------------------------------------------------------------

/// JSON snapshot of the world.
#[wasm_bindgen]
pub fn snapshot_json() -> String {
    with_app(|app| app.snapshot_json())
}

/// Draw one frame right now (screenshot tests).
#[wasm_bindgen]
pub fn render_frame() {
    with_app(|app| app.frame(js_sys::Date::now()));
}

/// Advance the simulation by `seconds` of real time.
#[wasm_bindgen]
pub fn step_seconds(seconds: f64) {
    with_app(|app| app.world_mut().tick(seconds as f32));
}

/// Skip to the next phase of the day.
#[wasm_bindgen]
pub fn time_skip() {
    with_app(|app| app.world_mut().time_skip());
}

/// Set the simulated clock (`hours` in 0..24).
#[wasm_bindgen]
pub fn set_time(hours: f64) {
    with_app(|app| app.world_mut().set_hours(hours as f32));
}

/// Show / hide the HUD.
#[wasm_bindgen]
pub fn set_hud(visible: bool) {
    with_app(|app| app.world_mut().set_hud_visible(visible));
}

/// Jump to a camera boom preset (deterministic, for the runtime tests).
#[wasm_bindgen]
pub fn set_camera_index(index: f64) {
    with_app(|app| app.world_mut().set_camera_index(index as usize));
}

/// Synthesise a key press (`key` is a DOM key name: `"w"`, `"Shift"`, …).
#[wasm_bindgen]
pub fn press_key(key: &str, down: bool) {
    with_app(|app| app.world_mut().key(key, down));
}

/// Put the character back on the spawn point.
#[wasm_bindgen]
pub fn respawn() {
    with_app(|app| {
        let p = app.world().spawn();
        app.world_mut().teleport(p);
    });
}

/// Frames drawn so far (`0` before boot).
#[wasm_bindgen]
pub fn frame_count() -> f64 {
    with_app(|app| app.frames() as f64)
}

/// `true` once at least one frame was drawn.
#[wasm_bindgen]
pub fn is_ready() -> bool {
    with_app(|app| app.is_ready())
}

/// Last renderer error (empty when healthy).
#[wasm_bindgen]
pub fn last_error() -> String {
    with_app(|app| app.error().unwrap_or_default())
}
