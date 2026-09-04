//! Browser glue: WebGL2 renderer, Canvas2D HUD, DOM events and the rAF loop.
//!
//! The simulation lives in [`crate::world`]; this module is only about pixels and events.
//! It only compiles for the wasm target (the browser is the only place WebGL2 exists);
//! the native build — and therefore `cargo test` — never sees any of it.

#![forbid(unsafe_code)]

use crate::shaders::{CITY_FS, CITY_VS, SKY_FS, SKY_VS};
use crate::world::{World, WorldConfig};
use city_math::{Mat4, Vec3};
use wasm_bindgen::prelude::*;
use web_sys::{
    Document, HtmlCanvasElement, HtmlElement, WebGl2RenderingContext as Gl, WebGlBuffer,
    WebGlProgram, WebGlShader, WebGlVertexArrayObject,
};

/// Vertex = pos(3) + normal(3) + colour(3).
pub(crate) const FLOATS_PER_VERTEX: usize = 9;
const STRIDE: i32 = (FLOATS_PER_VERTEX * 4) as i32;

/// Look up `#city`, build the renderer, HUD overlay and event plumbing.
pub fn start() -> Result<App, String> {
    let doc: Document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let canvas: HtmlCanvasElement = doc
        .get_element_by_id("city")
        .ok_or("missing <canvas id=\"city\">")?
        .dyn_into()
        .map_err(|_| "#city is not a canvas")?;
    App::new(canvas, &doc)
}

/// Owns the GL state, both canvases and the world.
pub struct App {
    world: World,
    canvas: HtmlCanvasElement,
    hud: HtmlCanvasElement,
    hud_ctx: web_sys::CanvasRenderingContext2d,
    gl: Gl,
    city_program: WebGlProgram,
    sky_program: WebGlProgram,
    empty_vao: Option<WebGlVertexArrayObject>,
    city_vao: Option<WebGlVertexArrayObject>,
    _city_vbo: Option<WebGlBuffer>,
    city_count: i32,
    /// Dynamic VAO holding the crowd and the traffic (re-uploaded every frame).
    agent_vao: Option<WebGlVertexArrayObject>,
    _agent_vbo: Option<WebGlBuffer>,
    agent_count: i32,
    last_ms: f64,
    ready: bool,
    error: Option<String>,
}

impl App {
    fn new(canvas: HtmlCanvasElement, doc: &Document) -> Result<App, String> {
        // `preserveDrawingBuffer` keeps the back buffer readable, so the page (and the
        // headless runtime tests) can read the rendered frame back with `drawImage` /
        // `toDataURL`. Without it the compositor owns the buffer and read-back is empty.
        let mut gl_attrs = web_sys::WebGlContextAttributes::new();
        gl_attrs.set_preserve_drawing_buffer(true);
        gl_attrs.set_antialias(true);
        let gl: Gl = canvas
            .get_context_with_context_options("webgl2", &gl_attrs)
            .map_err(|e| js_to_string(&e))?
            .ok_or("WebGL2 unavailable")?
            .dyn_into()
            .map_err(|_| "WebGL2 unavailable")?;

        let hud: HtmlCanvasElement = doc
            .create_element("canvas")
            .map_err(|e| js_to_string(&e))?
            .dyn_into()
            .map_err(|_| "cannot create the HUD canvas")?;
        hud_style(&canvas, &hud)?;

        let hud_ctx: web_sys::CanvasRenderingContext2d = hud
            .get_context("2d")
            .map_err(|e| js_to_string(&e))?
            .ok_or("no 2D context")?
            .dyn_into()
            .map_err(|_| "2D context unavailable")?;

        let city_program = build_program(&gl, CITY_VS, CITY_FS)?;
        let sky_program = build_program(&gl, SKY_VS, SKY_FS)?;

        let mut app = App {
            world: World::new(WorldConfig::default()),
            canvas,
            hud,
            hud_ctx,
            gl,
            city_program,
            sky_program,
            empty_vao: None,
            city_vao: None,
            _city_vbo: None,
            city_count: 0,
            agent_vao: None,
            _agent_vbo: None,
            agent_count: 0,
            last_ms: 0.0,
            ready: false,
            error: None,
        };
        app.upload_city();
        app.resize();
        Ok(app)
    }

    // --- public ---------------------------------------------------------

    pub fn world(&self) -> &World {
        &self.world
    }
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }
    pub fn is_ready(&self) -> bool {
        self.ready
    }
    /// JSON snapshot of the world (browser diagnostics).
    pub fn snapshot_json(&self) -> String {
        self.world.snapshot_json()
    }
    /// The simulated crowd as JSON (used by the runtime tests).
    pub fn crowd_json(&self) -> String {
        self.world.crowd_json()
    }
    /// Frames drawn so far.
    pub fn frames(&self) -> u64 {
        self.world.frames()
    }
    /// Last renderer error, `None` while all is well.
    pub fn error(&self) -> Option<String> {
        self.error.clone()
    }

    /// Match both canvases to the CSS box of the GL canvas, in device pixels.
    pub fn resize(&mut self) {
        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);
        let css_w = self.canvas.client_width().max(1);
        let css_h = self.canvas.client_height().max(1);
        let w = ((css_w as f64) * dpr).round().max(1.0) as u32;
        let h = ((css_h as f64) * dpr).round().max(1.0) as u32;
        if self.canvas.width() != w {
            self.canvas.set_width(w);
        }
        if self.canvas.height() != h {
            self.canvas.set_height(h);
        }
        self.hud.set_width(w);
        self.hud.set_height(h);
        self.world.set_dpr(dpr as f32);
    }

    /// Advance and draw one frame (`now_ms` from `requestAnimationFrame`).
    pub fn frame(&mut self, now_ms: f64) {
        let dt = if self.last_ms <= 0.0 {
            1.0 / 60.0
        } else {
            (now_ms - self.last_ms) / 1000.0
        };
        self.last_ms = now_ms;
        self.world.tick(dt as f32);
        self.upload_agents();
        self.error = self.render().err();
        self.draw_hud();
        self.ready = true;
    }

    // --- WebGL ----------------------------------------------------------

    fn render(&self) -> Result<(), String> {
        let sky = self.world.sample();
        let w = self.gl.drawing_buffer_width();
        let h = self.gl.drawing_buffer_height().max(1);
        self.gl.viewport(0, 0, self.canvas.width() as i32, self.canvas.height() as i32);
        self.gl
            .clear_color(sky.fog[0], sky.fog[1], sky.fog[2], 1.0);
        self.gl.clear(Gl::COLOR_BUFFER_BIT | Gl::DEPTH_BUFFER_BIT);

        let camera = self.world.camera();
        let view = camera.view();
        let proj = camera.projection(w as f32 / h.max(1) as f32);
        let eye = camera.eye();

        // ---- sky -------------------------------------------------------
        self.gl.disable(Gl::DEPTH_TEST);
        self.gl.depth_mask(false);
        self.gl.disable(Gl::CULL_FACE);
        self.gl.use_program(Some(&self.sky_program));
        set_mat4(&self.gl, &self.sky_program, "u_view_inv", &view_inverse(&view));
        set_vec3(&self.gl, &self.sky_program, "u_eye", eye);
        set_rgb(&self.gl, &self.sky_program, "u_zenith", sky.zenith);
        set_rgb(&self.gl, &self.sky_program, "u_horizon", sky.horizon);
        set_vec3(&self.gl, &self.sky_program, "u_sun_dir", sky.sun);
        set_rgb(&self.gl, &self.sky_program, "u_glow", sky.glow);
        set_f32(&self.gl, &self.sky_program, "u_exposure", sky.exposure);
        self.gl.bind_vertex_array(self.empty_vao.as_ref());
        self.gl.draw_arrays(Gl::TRIANGLES, 0, 3);

        // ---- city ------------------------------------------------------
        if self.city_count > 0 {
            self.gl.enable(Gl::DEPTH_TEST);
            self.gl.depth_mask(true);
            self.gl.enable(Gl::CULL_FACE);
            self.gl.cull_face(Gl::BACK);
            self.gl.use_program(Some(&self.city_program));
            set_mat4(&self.gl, &self.city_program, "u_view", &view);
            set_mat4(&self.gl, &self.city_program, "u_proj", &proj);
            set_vec3(&self.gl, &self.city_program, "u_eye", eye);
            set_vec3(
                &self.gl,
                &self.city_program,
                "u_light_dir",
                Vec3::new(sky.sun.x, sky.sun.y.max(0.05), sky.sun.z),
            );
            set_rgb(&self.gl, &self.city_program, "u_light_color", sky.light_color);
            set_f32(&self.gl, &self.city_program, "u_ambient", sky.ambient);
            set_rgb(&self.gl, &self.city_program, "u_fog_color", sky.fog);
            set_f32(&self.gl, &self.city_program, "u_fog_dist", sky.fog_distance);
            set_f32(&self.gl, &self.city_program, "u_exposure", sky.exposure);
            self.gl.bind_vertex_array(self.city_vao.as_ref());
            self.gl.draw_arrays(Gl::TRIANGLES, 0, self.city_count);

            // ---- crowd + traffic (dynamic) -----------------------------
            if self.agent_count > 0 {
                self.gl.bind_vertex_array(self.agent_vao.as_ref());
                self.gl.draw_arrays(Gl::TRIANGLES, 0, self.agent_count);
            }
            self.gl.bind_vertex_array(None);
        }
        self.gl.use_program(None);
        if self.gl.get_error() != 0 {
            return Err(format!("gl error {}", self.gl.get_error()));
        }
        Ok(())
    }

    /// Rebuild the dynamic agent mesh (crowd + traffic) and upload it.
    fn upload_agents(&mut self) {
        let mut m = crate::mesh::MeshBuilder::new();
        let headlight = self.world.sample().headlight;
        crate::agents::build_agents(
            self.world.crowd().peds(),
            self.world.crowd().cars(),
            headlight,
            &mut m,
        );
        let verts = m.into_vec();
        self.agent_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
        let gl = &self.gl;
        if self.agent_vao.is_none() {
            let vao = gl.create_vertex_array();
            let vbo = gl.create_buffer();
            gl.bind_vertex_array(vao.as_ref());
            gl.bind_buffer(Gl::ARRAY_BUFFER, vbo.as_ref());
            gl.buffer_data_with_u8_array(Gl::ARRAY_BUFFER, &f32_bytes(&verts), Gl::DYNAMIC_DRAW);
            gl.enable_vertex_attrib_array(0);
            gl.vertex_attrib_pointer_with_i32(0, 3, Gl::FLOAT, false, STRIDE, 0);
            gl.enable_vertex_attrib_array(1);
            gl.vertex_attrib_pointer_with_i32(1, 3, Gl::FLOAT, false, STRIDE, 12);
            gl.enable_vertex_attrib_array(2);
            gl.vertex_attrib_pointer_with_i32(2, 3, Gl::FLOAT, false, STRIDE, 24);
            gl.bind_vertex_array(None);
            self.agent_vao = vao;
            self._agent_vbo = vbo;
            return;
        }
        // Steady state: only the buffer contents change.
        let vbo = self._agent_vbo.clone();
        gl.bind_buffer(Gl::ARRAY_BUFFER, vbo.as_ref());
        gl.buffer_data_with_u8_array(Gl::ARRAY_BUFFER, &f32_bytes(&verts), Gl::DYNAMIC_DRAW);
    }

    /// (Re)build the static city mesh.
    fn upload_city(&mut self) {
        let mut m = crate::mesh::MeshBuilder::new();
        crate::mesh::build_city(self.world.city(), &mut m);
        let verts = m.into_vec();
        let gl = &self.gl;
        let vao = gl.create_vertex_array();
        gl.bind_vertex_array(vao.as_ref());
        let vbo = gl.create_buffer();
        gl.bind_buffer(Gl::ARRAY_BUFFER, vbo.as_ref());
        let bytes = f32_bytes(&verts);
        gl.buffer_data_with_u8_array(Gl::ARRAY_BUFFER, &bytes, Gl::STATIC_DRAW);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 3, Gl::FLOAT, false, STRIDE, 0);
        gl.enable_vertex_attrib_array(1);
        gl.vertex_attrib_pointer_with_i32(1, 3, Gl::FLOAT, false, STRIDE, 12);
        gl.enable_vertex_attrib_array(2);
        gl.vertex_attrib_pointer_with_i32(2, 3, Gl::FLOAT, false, STRIDE, 24);
        gl.bind_vertex_array(None);
        self.city_vao = vao;
        self._city_vbo = vbo;
        self.city_count = (verts.len() / FLOATS_PER_VERTEX) as i32;
    }

    // --- HUD ------------------------------------------------------------

    fn draw_hud(&self) {
        let ctx = &self.hud_ctx;
        let dpr = self.world.dpr().max(1.0) as f64;
        let w = self.hud.width() as f64;
        let h = self.hud.height() as f64;
        let _ = ctx.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        ctx.clear_rect(0.0, 0.0, w, h);
        if !self.world.hud_visible() {
            // "/" hides the HUD: the overlay stays cleared, nothing is painted
            return;
        }
        let mut f = self.world.hud_frame();
        // Street life on the radar: the crowd and the traffic within range.
        let crowd = self.world.crowd();
        let radar = self.world.radar();
        for ped in crowd.peds() {
            if radar.outside(city_math::Vec2::new(ped.x, ped.z)) {
                continue;
            }
            f.dots.push(city_hud::HudDot {
                p: radar.project(city_math::Vec2::new(ped.x, ped.z)),
                size: city_hud::dot_size(city_hud::HudDotKind::Ped),
                kind: city_hud::HudDotKind::Ped,
            });
        }
        for car in crowd.cars() {
            if radar.outside(car.pos) {
                continue;
            }
            f.dots.push(city_hud::HudDot {
                p: radar.project(car.pos),
                size: city_hud::dot_size(city_hud::HudDotKind::Car),
                kind: city_hud::HudDotKind::Car,
            });
        }
        let _ = ctx.set_transform(dpr, 0.0, 0.0, dpr, 0.0, 0.0);
        paint_hud(ctx, &f, (w / dpr) as f32, (h / dpr) as f32);
    }

    /// Draw one frame right now (used by `wasm.step_frame()` and the screenshot tests).
    pub fn render_once(&mut self) {
        self.upload_agents();
        self.error = self.render().err();
        self.draw_hud();
        self.ready = true;
    }
}

// ---------------------------------------------------------------------------
// HUD painting (vector drawing; no fonts are shipped with the app)
// ---------------------------------------------------------------------------

fn paint_hud(ctx: &web_sys::CanvasRenderingContext2d, f: &city_hud::HudFrame, wf: f32, hf: f32) {
    let w = wf as f64;
    let h = hf as f64;
    // ---- radar ---------------------------------------------------------
    let r = (104.0f32.min(wf * 0.22)) as f64;
    let cx = 18.0 + r;
    let cy = h - 18.0 - r;
    radar(ctx, f, r, cx, cy);

    // ---- clock ---------------------------------------------------------
    ctx.set_text_align("left");
    ctx.set_text_baseline("alphabetic");
    ctx.set_font("bold 30px ui-monospace, monospace");
    ctx.set_fill_style(&JsValue::from_str("rgba(6,10,20,0.55)"));
    let _ = ctx.fill_text(&f.clock, 22.0, 43.0);
    ctx.set_fill_style(&JsValue::from_str("rgba(228,244,255,0.96)"));
    let _ = ctx.fill_text(&f.clock, 20.0, 41.0);
    ctx.set_font("12px ui-sans-serif, system-ui, sans-serif");
    ctx.set_fill_style(&JsValue::from_str("rgba(150,222,255,0.9)"));
    let _ = ctx.fill_text(
        &format!("{} · facing {}", capitalize(&f.phase), f.compass),
        21.0,
        59.0,
    );

    // ---- speed ---------------------------------------------------------
    ctx.set_text_align("right");
    ctx.set_font("bold 26px ui-monospace, monospace");
    ctx.set_fill_style(&JsValue::from_str("rgba(228,244,255,0.92)"));
    let _ = ctx.fill_text(&format!("{:.0}", f.speed_kmh), w - 20.0, h - 36.0);
    ctx.set_font("11px ui-monospace, monospace");
    ctx.set_fill_style(&JsValue::from_str("rgba(150,205,235,0.8)"));
    let _ = ctx.fill_text(
        &format!(
            "km/h{} · cam {}/{}",
            if f.sprinting { " · sprint" } else { "" },
            f.cam_index,
            f.cam_count
        ),
        w - 20.0,
        h - 22.0,
    );

    // ---- context tip ---------------------------------------------------
    if !f.tip.is_empty() {
        ctx.set_text_align("left");
        ctx.set_font("13px ui-sans-serif, system-ui, sans-serif");
        ctx.set_fill_style(&JsValue::from_str("rgba(255,240,196,0.85)"));
        let _ = ctx.fill_text(&f.tip, 20.0, h - 82.0);
    }

    // ---- help line while the mouse is free ------------------------------
    if !f.locked {
        let line = "click to capture the mouse — WASD walk · Shift sprint · Space jump · F camera · T time skip · H hud";
        ctx.set_font("bold 13px ui-sans-serif, system-ui, sans-serif");
        let tw = measure(ctx, line);
        let bx = (w - tw) * 0.5 - 12.0;
        let by = h - 46.0;
        ctx.set_fill_style(&JsValue::from_str("rgba(8,12,24,0.68)"));
        ctx.begin_path();
        ctx.rect(bx as f64, by as f64, (tw + 24.0) as f64, 28.0);
        ctx.fill();
        ctx.set_fill_style(&JsValue::from_str("rgba(255,238,170,0.95)"));
        let _ = ctx.fill_text(line, (bx + 12.0) as f64, (by + 19.0) as f64);
    }
}

fn radar(ctx: &web_sys::CanvasRenderingContext2d, f: &city_hud::HudFrame, r: f64, cx: f64, cy: f64) {
    ctx.save();
    ctx.begin_path();
    let _ = ctx.arc(cx, cy, r, 0.0, std::f32::consts::TAU as f64);
    ctx.set_fill_style(&JsValue::from_str("rgba(6,11,22,0.82)"));
    ctx.fill();
    ctx.set_line_width(2.0);
    ctx.set_stroke_style(&JsValue::from_str("rgba(120,220,255,0.5)"));
    ctx.stroke();

    ctx.save();
    ctx.begin_path();
    let _ = ctx.arc(cx, cy, r - 1.0, 0.0, std::f32::consts::TAU as f64);
    ctx.clip();

    // parks under the streets
    for d in &f.dots {
        if d.kind == city_hud::HudDotKind::Green {
            ctx.set_fill_style(&JsValue::from_str("rgba(52,126,66,0.9)"));
            dot(ctx, cx + (d.p[0] * r as f32) as f64, cy + (d.p[1] * r as f32) as f64, d.size as f64);
        }
    }

    for l in &f.lines {
        ctx.begin_path();
        ctx.move_to(cx + (l.a[0] * r as f32) as f64, cy + (l.a[1] * r as f32) as f64);
        ctx.line_to(cx + (l.b[0] * r as f32) as f64, cy + (l.b[1] * r as f32) as f64);
        ctx.set_stroke_style(&JsValue::from_str(if l.avenue {
            "rgba(222,236,255,0.75)"
        } else {
            "rgba(140,172,205,0.45)"
        }));
        ctx.set_line_width(l.width as f64);
        ctx.stroke();
    }

    for d in &f.dots {
        let (col, size) = match d.kind {
            city_hud::HudDotKind::Green => continue,
            city_hud::HudDotKind::Player => ("rgba(255,242,150,1.0)", d.size * 1.25),
            city_hud::HudDotKind::Landmark => ("rgba(255,110,215,0.95)", d.size),
            city_hud::HudDotKind::Lamp => ("rgba(255,214,132,0.75)", d.size),
            city_hud::HudDotKind::Ped => ("rgba(255,150,150,0.95)", d.size),
            city_hud::HudDotKind::Car => ("rgba(150,210,255,0.95)", d.size),
        };
        ctx.set_fill_style(&JsValue::from_str(col));
        dot(ctx, cx + (d.p[0] * r as f32) as f64, cy + (d.p[1] * r as f32) as f64, size as f64);
    }
    ctx.restore();

    // heading tick at the top of the dial
    ctx.begin_path();
    ctx.move_to(cx, cy - r);
    ctx.line_to(cx - 6.0, cy - r + 10.0);
    ctx.line_to(cx + 6.0, cy - r + 10.0);
    ctx.close_path();
    ctx.set_fill_style(&JsValue::from_str("rgba(255,240,170,0.9)"));
    ctx.fill();
    ctx.restore();
}

/// Text width with a sane fallback (measurement is best-effort).
fn measure(ctx: &web_sys::CanvasRenderingContext2d, line: &str) -> f64 {
    // `CanvasRenderingContext2d::measure_text` is behind an extra web-sys feature;
    // a rough estimate is plenty for centring one help line.
    let _ = ctx;
    line.len() as f64 * 6.6
}

fn dot(ctx: &web_sys::CanvasRenderingContext2d, x: f64, y: f64, r: f64) {
    ctx.begin_path();
    let _ = ctx.arc(x, y, r.max(1.0), 0.0, std::f32::consts::TAU as f64);
    ctx.fill();
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Position the HUD canvas exactly above the GL canvas.
fn hud_style(canvas: &HtmlCanvasElement, hud: &HtmlCanvasElement) -> Result<(), String> {
    let parent = canvas.parent_element().ok_or("canvas has no parent")?;
    let host: &HtmlElement = canvas.unchecked_ref();
    let host_style = host.style();
    let _ = host_style.set_property("position", "absolute");
    let _ = host_style.set_property("left", "0px");
    let _ = host_style.set_property("top", "0px");
    let h: &HtmlElement = hud.unchecked_ref();
    hud.set_id("hud");
    let _ = h.style().set_property("position", "absolute");
    let _ = h.style().set_property("left", "0px");
    let _ = h.style().set_property("top", "0px");
    let _ = h.style().set_property("pointer-events", "none");
    parent.append_child(hud).map_err(|e| js_to_string(&e))?;
    Ok(())
}

fn build_program(gl: &Gl, vs_src: &str, fs_src: &str) -> Result<WebGlProgram, String> {
    let vs = compile_shader(gl, Gl::VERTEX_SHADER, vs_src)?;
    let fs = compile_shader(gl, Gl::FRAGMENT_SHADER, fs_src)?;
    let prog = gl.create_program().ok_or("cannot create program")?;
    gl.attach_shader(&prog, &vs);
    gl.attach_shader(&prog, &fs);
    gl.link_program(&prog);
    let ok = gl
        .get_program_parameter(&prog, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(prog)
    } else {
        Err(format!("program link failed: {:?}", gl.get_program_info_log(&prog)))
    }
}

fn compile_shader(gl: &Gl, kind: u32, src: &str) -> Result<WebGlShader, String> {
    let sh = gl.create_shader(kind).ok_or("cannot create shader")?;
    gl.shader_source(&sh, src);
    gl.compile_shader(&sh);
    let ok = gl
        .get_shader_parameter(&sh, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false);
    if ok {
        Ok(sh)
    } else {
        Err(format!("shader error: {:?}", gl.get_shader_info_log(&sh)))
    }
}

fn set_mat4(gl: &Gl, p: &WebGlProgram, name: &str, m: &Mat4) {
    let loc = gl.get_uniform_location(p, name);
    gl.uniform_matrix4fv_with_f32_array(loc.as_ref(), false, &m.to_flat());
}

fn set_vec3(gl: &Gl, p: &WebGlProgram, name: &str, v: Vec3) {
    let loc = gl.get_uniform_location(p, name);
    gl.uniform3f(loc.as_ref(), v.x, v.y, v.z);
}

fn set_rgb(gl: &Gl, p: &WebGlProgram, name: &str, c: [f32; 3]) {
    let loc = gl.get_uniform_location(p, name);
    gl.uniform3f(loc.as_ref(), c[0], c[1], c[2]);
}

fn set_f32(gl: &Gl, p: &WebGlProgram, name: &str, v: f32) {
    let loc = gl.get_uniform_location(p, name);
    gl.uniform1f(loc.as_ref(), v);
}

/// Copy an `f32` buffer into a byte buffer for `bufferData` (no `unsafe`).
fn f32_bytes(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

fn js_to_string(v: &JsValue) -> String {
    v.as_string().unwrap_or_else(|| format!("{v:?}"))
}

/// Inverse of a rigid view matrix (`[Rᵀ | -Rᵀt]`).
fn view_inverse(view: &Mat4) -> Mat4 {
    let mut inv = Mat4::IDENTITY;
    for c in 0..3 {
        for r in 0..3 {
            inv.set(c, r, view.at(r, c));
        }
    }
    let t = [view.at(3, 0), view.at(3, 1), view.at(3, 2)];
    for row in 0..3 {
        inv.set(3, row, -(view.at(0, row) * t[0] + view.at(1, row) * t[1] + view.at(2, row) * t[2]));
    }
    inv
}
