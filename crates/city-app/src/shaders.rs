//! City vertex shader: world transform + attributes for the unlit-but-shaded pass.

pub const CITY_VS: &str = r#"#version 300 es
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
uniform mat4 u_view;
uniform mat4 u_proj;
uniform vec3 u_eye;
out vec3 v_normal;
out vec3 v_color;
out vec3 v_world;
void main() {
  vec4 world = vec4(a_pos, 1.0);
  v_world = world.xyz;
  v_normal = a_normal;
  v_color = a_color;
  gl_Position = u_proj * u_view * world;
}
"#;

/// City fragment shader: hemispheric light + sun, fog, cheap tonemap.
pub const CITY_FS: &str = r#"#version 300 es
precision highp float;
in vec3 v_normal;
in vec3 v_color;
in vec3 v_world;
out vec4 out_color;
uniform vec3 u_eye;
uniform vec3 u_light_dir;
uniform vec3 u_light_color;
uniform float u_ambient;
uniform vec3 u_fog_color;
uniform float u_fog_dist;
uniform float u_exposure;

void main() {
  vec3 n = normalize(v_normal);
  float hemi = 0.5 + 0.5 * n.y;
  float ndl = max(dot(n, normalize(u_light_dir)), 0.0);
  vec3 lit = v_color * (u_ambient * hemi + ndl * u_light_color);
  float d = distance(v_world, u_eye);
  float fog = 1.0 - exp(-pow(max(d, 0.0) / max(u_fog_dist, 1.0), 2.0));
  vec3 col = mix(lit, u_fog_color, clamp(fog, 0.0, 1.0));
  col = col / (col + vec3(0.9));
  col = pow(col, vec3(0.85));
  out_color = vec4(col * u_exposure, 1.0);
}
"#;

/// Sky background: a full-screen triangle with a gradient + sun disc.
pub const SKY_VS: &str = r#"#version 300 es
precision highp float;
out vec2 v_ndc;
void main() {
  vec2 p = vec2(float((gl_VertexID << 1) & 2), float(gl_VertexID & 2));
  v_ndc = p * 2.0 - 1.0;
  gl_Position = vec4(v_ndc, 0.9999, 1.0);
}
"#;

pub const SKY_FS: &str = r#"#version 300 es
precision highp float;
in vec2 v_ndc;
out vec4 out_color;
uniform mat4 u_view_inv;
uniform vec3 u_eye;
uniform vec3 u_zenith;
uniform vec3 u_horizon;
uniform vec3 u_sun_dir;
uniform vec3 u_glow;
uniform float u_exposure;

void main() {
  vec4 p = u_view_inv * vec4(v_ndc, 1.0, 1.0);
  vec3 dir = normalize(p.xyz / p.w - u_eye);
  float t = pow(clamp(dir.y, 0.0, 1.0), 0.6);
  vec3 col = mix(u_horizon, u_zenith, t);
  vec3 sd = normalize(u_sun_dir);
  float glow = pow(clamp(dot(dir, sd), 0.0, 1.0), 12.0);
  col += u_glow * glow * 1.6;
  if (sd.y > -0.06) {
    float disc = smoothstep(0.9986, 0.9995, dot(dir, sd));
    col += vec3(1.0, 0.95, 0.85) * disc * 4.0;
  }
  float g = clamp(length(col), 0.0, 8.0);
  col = col / (col + vec3(1.0));
  col = pow(col, vec3(1.0 / 2.2));
  out_color = vec4(col, 1.0);
}
"#;
