//! Draws the preview's faces on the graphics card with a depth buffer, so every pixel shows
//! whatever is nearest. Sorting whole faces far-to-near (as egui meshes must be drawn) lets a
//! big face cover parts of a small one in front of it: half a visor hidden behind the head, or
//! triangles flickering where two pieces meet.
//!
//! Two passes: the opaque texels first, writing depth; then the see-through ones, far to near,
//! blended over what's in front of them.

use std::cell::RefCell;
use std::sync::Arc;

use eframe::egui::{self, Pos2, Rect, TextureId};
use eframe::egui_glow;
use eframe::glow::{self, HasContext as _};

use super::model::{Face, V3};

/// View-space depth that maps to the near and far planes (the model is about 40 across).
const DEPTH_RANGE: f32 = 200.0;
/// Floats per vertex: clip x, y, z, then u, v, then r, g, b, a.
const STRIDE: usize = 9;

/// Paint `faces` (sorted far to near) into the painter's area, `project` giving each
/// view-space point's place on the screen.
pub fn draw(painter: &egui::Painter, faces: &[Face], project: &impl Fn(V3) -> Pos2) {
    let area = painter.clip_rect();
    if faces.is_empty() || !area.is_positive() {
        return;
    }
    let (vertices, batches) = vertices(faces, project, area);
    let callback = egui_glow::CallbackFn::new(move |_info, painter| {
        RENDERER.with(|cell| {
            let mut cell = cell.borrow_mut();
            let gl = painter.gl();
            if cell.is_none() {
                match unsafe { Renderer::new(gl) } {
                    Ok(renderer) => *cell = Some(renderer),
                    Err(e) => {
                        log::warn!("3D preview unavailable: {e}");
                        return;
                    }
                }
            }
            if let Some(renderer) = cell.as_ref() {
                unsafe { renderer.paint(gl, painter, &vertices, &batches) };
            }
        });
    });
    painter.add(egui::PaintCallback {
        rect: area,
        callback: Arc::new(callback),
    });
}

/// Consecutive faces with the same texture: (texture, first vertex, vertex count).
type Batch = (TextureId, i32, i32);

fn vertices(faces: &[Face], project: &impl Fn(V3) -> Pos2, area: Rect) -> (Vec<f32>, Vec<Batch>) {
    let mut out = Vec::with_capacity(faces.len() * 6 * STRIDE);
    let mut batches: Vec<Batch> = Vec::new();
    for face in faces {
        let start = (out.len() / STRIDE) as i32;
        let rgba = egui::Rgba::from(face.tint);
        let color = [rgba.r(), rgba.g(), rgba.b(), rgba.a()];
        for i in [0, 1, 2, 0, 2, 3] {
            let corner = face.corners[i];
            let at = project(corner);
            out.extend_from_slice(&[
                (at.x - area.left()) / area.width() * 2.0 - 1.0,
                1.0 - (at.y - area.top()) / area.height() * 2.0,
                (-corner[2] / DEPTH_RANGE).clamp(-1.0, 1.0),
                face.uv[i].x,
                face.uv[i].y,
            ]);
            out.extend_from_slice(&color);
        }
        match batches.last_mut() {
            Some((texture, _, count)) if *texture == face.texture => *count += 6,
            _ => batches.push((face.texture, start, 6)),
        }
    }
    (out, batches)
}

thread_local! {
    /// Made on first use with the window's graphics context (only the main thread paints).
    static RENDERER: RefCell<Option<Renderer>> = const { RefCell::new(None) };
}

struct Renderer {
    program: glow::Program,
    buffer: glow::Buffer,
    vao: Option<glow::VertexArray>,
    pass: glow::UniformLocation,
    attributes: [(u32, i32, usize); 3],
}

impl Renderer {
    unsafe fn new(gl: &glow::Context) -> Result<Self, String> {
        let version = egui_glow::ShaderVersion::get(gl);
        let (vin, vout, fin, frag) = if version.is_new_shader_interface() {
            (
                "in",
                "out",
                "in",
                "out vec4 f_color;\n#define gl_FragColor f_color\n#define texture2D texture\n",
            )
        } else {
            ("attribute", "varying", "varying", "")
        };
        let precision = "#ifdef GL_ES\nprecision mediump float;\n#endif\n";
        let vertex = format!(
            "{}{precision}{vin} vec3 a_pos;\n{vin} vec2 a_uv;\n{vin} vec4 a_color;\n\
             {vout} vec2 v_uv;\n{vout} vec4 v_color;\n\
             void main() {{ v_uv = a_uv; v_color = a_color; gl_Position = vec4(a_pos, 1.0); }}\n",
            version.version_declaration()
        );
        // Colors are premultiplied and in gamma space, as egui paints them.
        let fragment = format!(
            "{}{precision}{frag}uniform sampler2D u_sampler;\nuniform float u_pass;\n\
             {fin} vec2 v_uv;\n{fin} vec4 v_color;\n\
             void main() {{\n\
               vec4 c = v_color * texture2D(u_sampler, v_uv);\n\
               if (u_pass < 0.5 ? c.a < 0.99 : (c.a >= 0.99 || c.a < 0.01)) discard;\n\
               gl_FragColor = c;\n\
             }}\n",
            version.version_declaration()
        );
        unsafe {
            let depth_bits = gl.get_framebuffer_attachment_parameter_i32(
                glow::FRAMEBUFFER,
                glow::DEPTH,
                glow::FRAMEBUFFER_ATTACHMENT_DEPTH_SIZE,
            );
            log::info!("3D preview: {depth_bits}-bit depth buffer");
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, source) in [
                (glow::VERTEX_SHADER, &vertex),
                (glow::FRAGMENT_SHADER, &fragment),
            ] {
                let shader = gl.create_shader(kind)?;
                gl.shader_source(shader, source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    return Err(gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                return Err(gl.get_program_info_log(program));
            }
            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }
            let attribute = |name: &str| {
                gl.get_attrib_location(program, name)
                    .ok_or_else(|| format!("no {name} in the preview shader"))
            };
            let attributes = [
                (attribute("a_pos")?, 3, 0),
                (attribute("a_uv")?, 2, 3),
                (attribute("a_color")?, 4, 5),
            ];
            Ok(Self {
                pass: gl
                    .get_uniform_location(program, "u_pass")
                    .ok_or("no u_pass in the preview shader")?,
                buffer: gl.create_buffer()?,
                vao: gl.create_vertex_array().ok(),
                program,
                attributes,
            })
        }
    }

    unsafe fn paint(
        &self,
        gl: &glow::Context,
        painter: &egui_glow::Painter,
        vertices: &[f32],
        batches: &[Batch],
    ) {
        unsafe {
            gl.use_program(Some(self.program));
            if let Some(vao) = self.vao {
                gl.bind_vertex_array(Some(vao));
            }
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.buffer));
            gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck_f32(vertices),
                glow::STREAM_DRAW,
            );
            let stride = (STRIDE * size_of::<f32>()) as i32;
            for (location, size, offset) in self.attributes {
                gl.enable_vertex_attrib_array(location);
                gl.vertex_attrib_pointer_f32(
                    location,
                    size,
                    glow::FLOAT,
                    false,
                    stride,
                    (offset * size_of::<f32>()) as i32,
                );
            }
            gl.active_texture(glow::TEXTURE0);
            // The scissor is already the painter's area: only it is cleared.
            gl.depth_mask(true);
            gl.clear_depth_f32(1.0);
            gl.clear(glow::DEPTH_BUFFER_BIT);
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.enable(glow::BLEND);
            gl.blend_func(glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
            for (pass, writes_depth) in [(0.0, true), (1.0, false)] {
                gl.uniform_1_f32(Some(&self.pass), pass);
                gl.depth_mask(writes_depth);
                for &(texture, first, count) in batches {
                    let Some(texture) = painter.texture(texture) else {
                        continue;
                    };
                    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                    gl.draw_arrays(glow::TRIANGLES, first, count);
                }
            }
            gl.depth_mask(true);
            gl.disable(glow::DEPTH_TEST);
            for (location, _, _) in self.attributes {
                gl.disable_vertex_attrib_array(location);
            }
            if self.vao.is_some() {
                gl.bind_vertex_array(None);
            }
        }
    }
}

fn bytemuck_f32(data: &[f32]) -> &[u8] {
    // SAFETY: f32 has no padding or invalid bit patterns as bytes, and u8 has alignment 1.
    unsafe { std::slice::from_raw_parts(data.as_ptr().cast(), std::mem::size_of_val(data)) }
}
