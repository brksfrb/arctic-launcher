//! The Glass theme's frosted panes: the backdrop, blurred, seen through each card and the sidebar.
//!
//! Panes are found once the UI is laid out ([`note_panes`]); one paint callback, drawn right after the
//! backdrop, copies what is on screen, blurs it at a fraction of the window's size and draws the
//! blurred copy inside each pane with its rounded corners. The cards' own light tint and edge are
//! painted over it as usual. Only the Glass theme asks for any of this.

use std::sync::{Arc, Mutex, PoisonError};

use eframe::egui::{self, Color32, Rect};
use eframe::egui_glow;
use eframe::glow::{self, HasContext};

/// The blur runs at 1/DOWNSCALE of the window's size: cheap, and blurrier for free.
const DOWNSCALE: i32 = 4;
/// Horizontal + vertical blur passes.
const PASSES: usize = 2;

/// A frosted area noted this frame: where, how round, clipped to what, how faded in.
struct Pane {
    rect: Rect,
    radius: f32,
    clip: Rect,
    opacity: f32,
}

static PANES: Mutex<Vec<Pane>> = Mutex::new(Vec::new());
static GL: Mutex<Option<Blur>> = Mutex::new(None);

/// Note this frame's panes: every rectangle painted in one of `fills` (the theme's card and
/// sidebar colors), at any opacity, with its rounding and clip. Call once the UI is laid out,
/// before it's painted; the backdrop callback reads them.
pub fn note_panes(ctx: &egui::Context, fills: &[Color32]) {
    let mut found = Vec::new();
    // Asked first: the context can't be asked anything while its graphics are borrowed.
    let ids: Vec<egui::LayerId> = ctx.memory(|m| m.layer_ids().collect());
    ctx.graphics(|layers| {
        for layer in ids {
            let Some(list) = layers.get(layer) else {
                continue;
            };
            for clipped in list.all_entries() {
                collect(&clipped.shape, clipped.clip_rect, fills, &mut found);
            }
        }
    });
    *PANES.lock().unwrap_or_else(PoisonError::into_inner) = found;
}

fn collect(shape: &egui::Shape, clip: Rect, fills: &[Color32], out: &mut Vec<Pane>) {
    match shape {
        egui::Shape::Vec(shapes) => {
            for s in shapes {
                collect(s, clip, fills, out);
            }
        }
        egui::Shape::Rect(r) => {
            if let Some(opacity) = fills.iter().find_map(|f| faded(r.fill, *f)) {
                out.push(Pane {
                    rect: r.rect,
                    radius: f32::from(r.corner_radius.nw),
                    clip,
                    opacity,
                });
            }
        }
        _ => {}
    }
}

/// How faded `color` is relative to `fill` (premultiplied: fading scales every channel), if it is
/// `fill` at some opacity.
fn faded(color: Color32, fill: Color32) -> Option<f32> {
    if fill.a() == 0 || color.a() == 0 {
        return None;
    }
    let t = f32::from(color.a()) / f32::from(fill.a());
    let close = |c: u8, f: u8| (f32::from(c) - f32::from(f) * t).abs() <= 2.0;
    (t <= 1.01
        && close(color.r(), fill.r())
        && close(color.g(), fill.g())
        && close(color.b(), fill.b()))
    .then_some(t.min(1.0))
}

/// The callback that draws every pane: paint it over the whole window, right after the backdrop.
pub fn backdrop_callback(screen: Rect) -> egui::PaintCallback {
    egui::PaintCallback {
        rect: screen,
        callback: Arc::new(egui_glow::CallbackFn::new(|info, painter| {
            let panes: Vec<(Rect, f32, Rect, f32)> = PANES
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .iter()
                .map(|p| (p.rect, p.radius, p.clip, p.opacity))
                .collect();
            if panes.is_empty() {
                return;
            }
            let gl = painter.gl();
            let mut slot = GL.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.is_none() {
                match unsafe { Blur::new(gl) } {
                    Ok(blur) => *slot = Some(blur),
                    Err(e) => {
                        log::warn!("glass blur unavailable: {e}");
                        return;
                    }
                }
            }
            if let Some(blur) = slot.as_mut() {
                unsafe { blur.draw(gl, &info, &panes) };
            }
        })),
    }
}

struct Target {
    texture: glow::Texture,
    framebuffer: glow::Framebuffer,
}

struct Blur {
    copy: glow::Program,
    blur: glow::Program,
    composite: glow::Program,
    quad: glow::Buffer,
    vao: glow::VertexArray,
    /// The screen as painted so far, full size.
    screen: Option<glow::Texture>,
    /// Ping-pong targets at 1/DOWNSCALE size.
    small: [Option<Target>; 2],
    size: (i32, i32),
}

const VERTEX: &str = "
ATTR vec2 pos;
uniform vec4 u_rect;
VARY_OUT vec2 v_uv;
void main() {
    v_uv = pos;
    gl_Position = vec4(mix(u_rect.xy, u_rect.zw, pos), 0.0, 1.0);
}";

const COPY: &str = "
uniform sampler2D u_tex;
uniform vec2 u_texel;
VARY_IN vec2 v_uv;
void main() {
    // Four taps a texel apart: a box filter for the shrink.
    vec4 c = TEX(u_tex, v_uv + vec2(-u_texel.x, -u_texel.y))
           + TEX(u_tex, v_uv + vec2(u_texel.x, -u_texel.y))
           + TEX(u_tex, v_uv + vec2(-u_texel.x, u_texel.y))
           + TEX(u_tex, v_uv + vec2(u_texel.x, u_texel.y));
    FRAG = c * 0.25;
}";

const BLUR: &str = "
uniform sampler2D u_tex;
uniform vec2 u_step;
VARY_IN vec2 v_uv;
void main() {
    vec4 c = TEX(u_tex, v_uv) * 0.227027;
    c += (TEX(u_tex, v_uv + u_step * 1.3846) + TEX(u_tex, v_uv - u_step * 1.3846)) * 0.316216;
    c += (TEX(u_tex, v_uv + u_step * 3.2308) + TEX(u_tex, v_uv - u_step * 3.2308)) * 0.070270;
    FRAG = c;
}";

const COMPOSITE: &str = "
uniform sampler2D u_tex;
uniform vec2 u_screen;
uniform vec2 u_size;
uniform float u_radius;
uniform float u_opacity;
VARY_IN vec2 v_uv;
void main() {
    vec3 c = TEX(u_tex, gl_FragCoord.xy / u_screen).rgb;
    // A touch brighter and more colorful, like frosted glass.
    float grey = dot(c, vec3(0.299, 0.587, 0.114));
    c = mix(vec3(grey), c, 1.25) * 1.06;
    vec2 p = (v_uv - 0.5) * u_size;
    vec2 q = abs(p) - (u_size * 0.5 - vec2(u_radius));
    float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - u_radius;
    float a = clamp(0.5 - d, 0.0, 1.0) * u_opacity;
    FRAG = vec4(c * a, a);
}";

impl Blur {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        unsafe {
            let version = egui_glow::ShaderVersion::get(gl);
            let copy = program(gl, version, COPY)?;
            let blur = program(gl, version, BLUR)?;
            let composite = program(gl, version, COMPOSITE)?;
            let quad = gl.create_buffer()?;
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(quad));
            let corners: [f32; 8] = [0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0];
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes(&corners), glow::STATIC_DRAW);
            let vao = gl.create_vertex_array()?;
            gl.bind_vertex_array(Some(vao));
            for p in [copy, blur, composite] {
                if let Some(at) = gl.get_attrib_location(p, "pos") {
                    gl.enable_vertex_attrib_array(at);
                    gl.vertex_attrib_pointer_f32(at, 2, glow::FLOAT, false, 8, 0);
                }
            }
            Ok(Self {
                copy,
                blur,
                composite,
                quad,
                vao,
                screen: None,
                small: [None, None],
                size: (0, 0),
            })
        }
    }

    /// (Re)make the textures for a `w`×`h` window.
    unsafe fn resize(&mut self, gl: &glow::Context, w: i32, h: i32) -> Result<(), String> {
        unsafe {
            if self.size == (w, h) && self.screen.is_some() {
                return Ok(());
            }
            if let Some(t) = self.screen.take() {
                gl.delete_texture(t);
            }
            for t in self.small.iter_mut().filter_map(Option::take) {
                gl.delete_texture(t.texture);
                gl.delete_framebuffer(t.framebuffer);
            }
            self.screen = Some(texture(gl, w, h)?);
            let (sw, sh) = ((w / DOWNSCALE).max(1), (h / DOWNSCALE).max(1));
            for slot in &mut self.small {
                let texture = texture(gl, sw, sh)?;
                let framebuffer = gl.create_framebuffer()?;
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(framebuffer));
                gl.framebuffer_texture_2d(
                    glow::FRAMEBUFFER,
                    glow::COLOR_ATTACHMENT0,
                    glow::TEXTURE_2D,
                    Some(texture),
                    0,
                );
                *slot = Some(Target {
                    texture,
                    framebuffer,
                });
            }
            self.size = (w, h);
            Ok(())
        }
    }

    unsafe fn draw(
        &mut self,
        gl: &glow::Context,
        info: &egui::PaintCallbackInfo,
        panes: &[(Rect, f32, Rect, f32)],
    ) {
        unsafe {
            let [w, h] = info.screen_size_px.map(|v| v as i32);
            if w <= 0 || h <= 0 {
                return;
            }
            // egui paints into whatever framebuffer is bound: read from it, then put it back.
            let bound = gl.get_parameter_framebuffer(glow::FRAMEBUFFER_BINDING);
            if let Err(e) = self.resize(gl, w, h) {
                log::warn!("glass blur: {e}");
                gl.bind_framebuffer(glow::FRAMEBUFFER, bound);
                return;
            }
            gl.bind_framebuffer(glow::FRAMEBUFFER, bound);
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, self.screen);
            gl.copy_tex_sub_image_2d(glow::TEXTURE_2D, 0, 0, 0, 0, 0, w, h);

            gl.disable(glow::SCISSOR_TEST);
            gl.disable(glow::BLEND);
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.quad));
            let (sw, sh) = ((w / DOWNSCALE).max(1), (h / DOWNSCALE).max(1));
            let full = [-1.0, -1.0, 1.0, 1.0];
            let (Some(a), Some(b)) = (&self.small[0], &self.small[1]) else {
                return;
            };
            // Shrink the screen into A.
            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(a.framebuffer));
            gl.viewport(0, 0, sw, sh);
            gl.use_program(Some(self.copy));
            set_rect(gl, self.copy, full);
            set2(gl, self.copy, "u_texel", 1.0 / w as f32, 1.0 / h as f32);
            set_sampler(gl, self.copy);
            gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            // Blur A ↔ B, across then down.
            gl.use_program(Some(self.blur));
            set_rect(gl, self.blur, full);
            set_sampler(gl, self.blur);
            for _ in 0..PASSES {
                for (from, to, step) in [
                    (a, b, [1.0 / sw as f32, 0.0]),
                    (b, a, [0.0, 1.0 / sh as f32]),
                ] {
                    gl.bind_framebuffer(glow::FRAMEBUFFER, Some(to.framebuffer));
                    gl.bind_texture(glow::TEXTURE_2D, Some(from.texture));
                    set2(gl, self.blur, "u_step", step[0], step[1]);
                    gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
                }
            }
            // Each pane: the blurred backdrop inside its rounded rectangle.
            gl.bind_framebuffer(glow::FRAMEBUFFER, bound);
            gl.viewport(0, 0, w, h);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            gl.enable(glow::SCISSOR_TEST);
            gl.use_program(Some(self.composite));
            gl.bind_texture(glow::TEXTURE_2D, Some(a.texture));
            set_sampler(gl, self.composite);
            set2(gl, self.composite, "u_screen", w as f32, h as f32);
            let ppp = info.pixels_per_point;
            for (rect, radius, clip, opacity) in panes {
                let clip = clip.intersect(*rect);
                if !clip.is_positive() {
                    continue;
                }
                let x0 = (clip.min.x * ppp).floor() as i32;
                let x1 = (clip.max.x * ppp).ceil() as i32;
                let y0 = h - (clip.max.y * ppp).ceil() as i32;
                let y1 = h - (clip.min.y * ppp).floor() as i32;
                gl.scissor(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0));
                let ndc = |x: f32, y: f32| {
                    [
                        x * ppp / w as f32 * 2.0 - 1.0,
                        1.0 - y * ppp / h as f32 * 2.0,
                    ]
                };
                let [l, t] = ndc(rect.min.x, rect.min.y);
                let [r, bt] = ndc(rect.max.x, rect.max.y);
                // Quad corner (0, 0) is the rectangle's bottom-left.
                set_rect(gl, self.composite, [l, bt, r, t]);
                set2(
                    gl,
                    self.composite,
                    "u_size",
                    rect.width() * ppp,
                    rect.height() * ppp,
                );
                set1(gl, self.composite, "u_radius", radius * ppp);
                set1(gl, self.composite, "u_opacity", *opacity);
                gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, None);
            gl.bind_vertex_array(None);
        }
    }
}

unsafe fn program(
    gl: &glow::Context,
    version: egui_glow::ShaderVersion,
    fragment: &str,
) -> Result<glow::Program, String> {
    unsafe {
        let new = version.is_new_shader_interface();
        let precision = if version.version_declaration().contains("es") {
            "precision mediump float;\n"
        } else {
            ""
        };
        let vertex_head = if new {
            "#define ATTR in\n#define VARY_OUT out\n"
        } else {
            "#define ATTR attribute\n#define VARY_OUT varying\n"
        };
        let fragment_head = if new {
            "#define VARY_IN in\n#define TEX texture\nout vec4 arctic_frag;\n#define FRAG arctic_frag\n"
        } else {
            "#define VARY_IN varying\n#define TEX texture2D\n#define FRAG gl_FragColor\n"
        };
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, head, body) in [
            (glow::VERTEX_SHADER, vertex_head, VERTEX),
            (glow::FRAGMENT_SHADER, fragment_head, fragment),
        ] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(
                shader,
                &format!("{}{precision}{head}{body}", version.version_declaration()),
            );
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.bind_attrib_location(program, 0, "pos");
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for s in shaders {
            gl.detach_shader(program, s);
            gl.delete_shader(s);
        }
        Ok(program)
    }
}

unsafe fn texture(gl: &glow::Context, w: i32, h: i32) -> Result<glow::Texture, String> {
    unsafe {
        let t = gl.create_texture()?;
        gl.bind_texture(glow::TEXTURE_2D, Some(t));
        gl.tex_image_2d(
            glow::TEXTURE_2D,
            0,
            glow::RGBA as i32,
            w,
            h,
            0,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelUnpackData::Slice(None),
        );
        for (p, v) in [
            (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
            (glow::TEXTURE_MAG_FILTER, glow::LINEAR),
            (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
            (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
        ] {
            gl.tex_parameter_i32(glow::TEXTURE_2D, p, v as i32);
        }
        Ok(t)
    }
}

unsafe fn set_rect(gl: &glow::Context, p: glow::Program, r: [f32; 4]) {
    unsafe {
        gl.uniform_4_f32(
            gl.get_uniform_location(p, "u_rect").as_ref(),
            r[0],
            r[1],
            r[2],
            r[3],
        )
    }
}

unsafe fn set2(gl: &glow::Context, p: glow::Program, name: &str, x: f32, y: f32) {
    unsafe { gl.uniform_2_f32(gl.get_uniform_location(p, name).as_ref(), x, y) }
}

unsafe fn set1(gl: &glow::Context, p: glow::Program, name: &str, x: f32) {
    unsafe { gl.uniform_1_f32(gl.get_uniform_location(p, name).as_ref(), x) }
}

unsafe fn set_sampler(gl: &glow::Context, p: glow::Program) {
    unsafe { gl.uniform_1_i32(gl.get_uniform_location(p, "u_tex").as_ref(), 0) }
}

fn bytes(v: &[f32]) -> &[u8] {
    // SAFETY: f32s are plain data; the slice covers exactly their bytes.
    unsafe { std::slice::from_raw_parts(v.as_ptr().cast(), std::mem::size_of_val(v)) }
}
