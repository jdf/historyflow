//! Draws a layout with WebGL2: every quad is one instance of a unit
//! square, placed by the vertex shader.

use historyflow_core::{Layout, Quad, View, YScale};
use wasm_bindgen::prelude::*;
use web_sys::{
    HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlBuffer, WebGlProgram, WebGlShader,
    WebGlTexture, WebGlUniformLocation,
};

/// What colour means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Coloring {
    /// Hue by author; text darkens as it ages.
    #[default]
    Author,
    /// No hue: white for new text through to dark grey for old.
    Age,
}

/// Authors per row of the palette texture.
const PALETTE_WIDTH: usize = 256;

/// CSS pixels left empty around the plot.
pub const MARGIN: f32 = 12.0;

/// What is drawn over the plot for the selected version.
pub struct Overlay {
    /// The crosshair, in layout coordinates.
    pub x: f32,
    pub y: f32,
    /// The x of each version to mark below the plot: those by the
    /// selected version's author.
    pub marks: Vec<f32>,
}

const WHITE: [f32; 3] = [1.0, 1.0, 1.0];
const ORANGE: [f32; 3] = [1.0, 0.55, 0.1];

/// Fills a rectangle given in pixels with one colour.
const SOLID_VERTEX_SHADER: &str = "#version 300 es
layout(location = 0) in vec2 corner;
uniform vec2 viewport;
uniform vec4 rect; // x, y, width, height
void main() {
    vec2 pixel = rect.xy + corner * rect.zw;
    gl_Position = vec4(
        pixel.x / viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / viewport.y * 2.0,
        0.0,
        1.0);
}";

const SOLID_FRAGMENT_SHADER: &str = "#version 300 es
precision mediump float;
uniform vec3 fill;
out vec4 color;
void main() {
    color = vec4(fill, 1.0);
}";

const VERTEX_SHADER: &str = "#version 300 es
// A corner of the unit square: x is 0 on the quad's left side and 1 on
// its right, y is 0 at the top and 1 at the bottom.
layout(location = 0) in vec2 corner;
// Per quad, in the layout's normalised coordinates.
layout(location = 1) in vec2 xs;      // left x, right x
layout(location = 2) in vec4 ys;      // left top, left bottom, right top, right bottom
layout(location = 3) in vec3 info;    // author, left age, right age

uniform vec2 viewport;    // canvas size in pixels
uniform vec4 plot;        // the plot's rectangle in pixels: x, y, width, height
uniform vec2 view;        // the layout x at the plot's left edge, and the span across it
uniform vec2 y_scale;     // the layout y at the plot's bottom edge, and the log steepness (0: linear)
uniform float line_width; // width of a version's line in pixels
uniform float age_span;   // the age, in versions, drawn as fully aged

flat out int author;
out float age;

// historyflow_core::YScale::down.
float curve(float y) {
    return y_scale.y > 0.0 ? log(1.0 + y_scale.y * y) : y;
}

void main() {
    bool right = corner.x > 0.5;
    float x = right ? xs.y : xs.x;
    float y = right ? mix(ys.z, ys.w, corner.y) : mix(ys.x, ys.y, corner.y);
    vec2 across = vec2((x - view.x) / view.y, curve(y) / curve(y_scale.x));
    vec2 pixel = plot.xy + across * plot.zw;
    // A version's own line has no width in layout coordinates.
    if (xs.x == xs.y) {
        pixel.x += (corner.x - 0.5) * line_width;
    }
    gl_Position = vec4(
        pixel.x / viewport.x * 2.0 - 1.0,
        1.0 - pixel.y / viewport.y * 2.0,
        0.0,
        1.0);
    author = int(info.x);
    age = (right ? info.z : info.y) / age_span;
}";

const FRAGMENT_SHADER: &str = "#version 300 es
precision mediump float;

uniform sampler2D palette; // one texel per author
uniform int coloring;      // 0: by author, 1: by age

flat in int author;
in float age;
out vec4 color;

void main() {
    float aged = clamp(age, 0.0, 1.0);
    if (coloring == 0) {
        vec3 hue = texelFetch(palette, ivec2(author % 256, author / 256), 0).rgb;
        color = vec4(hue * mix(1.0, 0.4, aged), 1.0);
    } else {
        color = vec4(vec3(mix(1.0, 0.16, aged)), 1.0);
    }
}";

pub struct Renderer {
    gl: Gl,
    program: WebGlProgram,
    solid: WebGlProgram,
    instances: WebGlBuffer,
    palette: WebGlTexture,
    quads: i32,
}

impl Renderer {
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Renderer, JsValue> {
        let gl: Gl = canvas
            .get_context("webgl2")?
            .ok_or("WebGL2 is not available")?
            .dyn_into()?;
        let program = link(&gl, VERTEX_SHADER, FRAGMENT_SHADER)?;
        let solid = link(&gl, SOLID_VERTEX_SHADER, SOLID_FRAGMENT_SHADER)?;

        let vao = gl.create_vertex_array().ok_or("create_vertex_array")?;
        gl.bind_vertex_array(Some(&vao));

        // The unit square as a triangle strip, shared by every instance.
        let corners = gl.create_buffer().ok_or("create_buffer")?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&corners));
        let square =
            js_sys::Float32Array::from([0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0].as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &square, Gl::STATIC_DRAW);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_with_i32(0, 2, Gl::FLOAT, false, 0, 0);

        // One Quad::floats() per instance.
        let instances = gl.create_buffer().ok_or("create_buffer")?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&instances));
        let stride = (Quad::FLOATS * 4) as i32;
        for (location, size, offset) in [(1, 2, 0), (2, 4, 2), (3, 3, 6)] {
            gl.enable_vertex_attrib_array(location);
            gl.vertex_attrib_pointer_with_i32(location, size, Gl::FLOAT, false, stride, offset * 4);
            gl.vertex_attrib_divisor(location, 1);
        }

        let palette = gl.create_texture().ok_or("create_texture")?;
        gl.bind_texture(Gl::TEXTURE_2D, Some(&palette));
        for parameter in [Gl::TEXTURE_MIN_FILTER, Gl::TEXTURE_MAG_FILTER] {
            gl.tex_parameteri(Gl::TEXTURE_2D, parameter, Gl::NEAREST as i32);
        }

        Ok(Renderer {
            gl,
            program,
            solid,
            instances,
            palette,
            quads: 0,
        })
    }

    /// Replaces what is drawn.
    pub fn upload(&mut self, layout: &Layout, colors: &[[f32; 3]]) -> Result<(), JsValue> {
        let gl = &self.gl;
        let floats: Vec<f32> = layout.quads.iter().flat_map(Quad::floats).collect();
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&self.instances));
        let data = js_sys::Float32Array::from(floats.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::STATIC_DRAW);
        self.quads = layout.quads.len() as i32;

        let rows = colors.len().div_ceil(PALETTE_WIDTH).max(1);
        let mut texels = vec![0u8; PALETTE_WIDTH * rows * 4];
        for (author, color) in colors.iter().enumerate() {
            let texel = &mut texels[author * 4..author * 4 + 4];
            for (byte, channel) in texel.iter_mut().zip(color) {
                *byte = (channel * 255.0).round() as u8;
            }
            texel[3] = 255;
        }
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.palette));
        gl.tex_image_2d_with_i32_and_i32_and_i32_and_format_and_type_and_opt_u8_array(
            Gl::TEXTURE_2D,
            0,
            Gl::RGBA8 as i32,
            PALETTE_WIDTH as i32,
            rows as i32,
            0,
            Gl::RGBA,
            Gl::UNSIGNED_BYTE,
            Some(&texels),
        )
    }

    pub fn draw(
        &self,
        coloring: Coloring,
        versions: usize,
        view: View,
        scale: YScale,
        pixel_ratio: f32,
        overlay: Option<&Overlay>,
    ) {
        let gl = &self.gl;
        let (width, height) = (
            gl.drawing_buffer_width() as f32,
            gl.drawing_buffer_height() as f32,
        );
        gl.viewport(0, 0, width as i32, height as i32);
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT);

        let margin = MARGIN * pixel_ratio;
        let (plot_width, plot_height) = (width - 2.0 * margin, height - 2.0 * margin);
        // A version's line is 2 CSS pixels wide, or less where versions
        // are packed so tightly that lines would hide the ribbons.
        let spacing = plot_width / (versions.max(2) as f32 * view.width);
        let line_width = (2.0 * pixel_ratio).min(spacing * 0.5).max(1.0);

        // Zoomed in, the plot continues past both sides, and with the
        // height fitted to the view a longer version just off screen runs
        // off the bottom; keep it all out of the margins (but for the
        // half of a line that overhangs them at the sides).
        gl.enable(Gl::SCISSOR_TEST);
        let overhang = line_width / 2.0;
        let (left, across) = (
            (margin - overhang).floor() as i32,
            (plot_width + 2.0 * overhang).ceil() as i32,
        );
        gl.scissor(
            left,
            margin.floor() as i32,
            across,
            plot_height.ceil() as i32,
        );

        gl.use_program(Some(&self.program));
        let uniform = |name| gl.get_uniform_location(&self.program, name);
        let set = |location: Option<WebGlUniformLocation>, values: &[f32]| match values {
            [a] => gl.uniform1f(location.as_ref(), *a),
            [a, b] => gl.uniform2f(location.as_ref(), *a, *b),
            [a, b, c, d] => gl.uniform4f(location.as_ref(), *a, *b, *c, *d),
            _ => unreachable!(),
        };
        set(uniform("viewport"), &[width, height]);
        set(uniform("plot"), &[margin, margin, plot_width, plot_height]);
        set(uniform("view"), &[view.start, view.width]);
        set(uniform("y_scale"), &[scale.top, scale.log]);
        set(uniform("line_width"), &[line_width]);
        set(uniform("age_span"), &[versions.max(2) as f32 - 1.0]);
        gl.uniform1i(uniform("coloring").as_ref(), coloring as i32);
        gl.uniform1i(uniform("palette").as_ref(), 0);
        gl.active_texture(Gl::TEXTURE0);
        gl.bind_texture(Gl::TEXTURE_2D, Some(&self.palette));
        if self.quads > 0 {
            gl.draw_arrays_instanced(Gl::TRIANGLE_STRIP, 0, 4, self.quads);
        }

        let Some(overlay) = overlay else {
            gl.disable(Gl::SCISSOR_TEST);
            return;
        };
        // The marks sit in the bottom margin.
        gl.scissor(left, 0, across, height as i32);
        gl.use_program(Some(&self.solid));
        let uniform = |name| gl.get_uniform_location(&self.solid, name);
        gl.uniform2f(uniform("viewport").as_ref(), width, height);
        let (rect, fill) = (uniform("rect"), uniform("fill"));
        let fill_rect = |x: f32, y: f32, w: f32, h: f32, color: [f32; 3]| {
            gl.uniform4f(
                rect.as_ref(),
                x.round(),
                y.round(),
                w.round().max(1.0),
                h.round().max(1.0),
            );
            gl.uniform3f(fill.as_ref(), color[0], color[1], color[2]);
            gl.draw_arrays(Gl::TRIANGLE_STRIP, 0, 4);
        };
        let thin = pixel_ratio;
        let dot = 3.0 * pixel_ratio;
        for &mark in &overlay.marks {
            let x = margin + view.across(mark) * plot_width;
            fill_rect(
                x - dot / 2.0,
                height - (margin + dot) / 2.0,
                dot,
                dot,
                WHITE,
            );
        }
        let x = margin + view.across(overlay.x) * plot_width;
        let y = margin + overlay.y * plot_height;
        fill_rect(x - thin / 2.0, margin, thin, plot_height, WHITE);
        // The crosshair's place may be off the bottom, when the height is
        // fitted to shorter versions than the selected one.
        if overlay.y <= 1.0 {
            fill_rect(margin, y - thin / 2.0, plot_width, thin, ORANGE);
        }
        gl.disable(Gl::SCISSOR_TEST);
    }
}

fn compile(gl: &Gl, kind: u32, source: &str) -> Result<WebGlShader, JsValue> {
    let shader = gl.create_shader(kind).ok_or("create_shader")?;
    gl.shader_source(&shader, source);
    gl.compile_shader(&shader);
    if gl
        .get_shader_parameter(&shader, Gl::COMPILE_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(shader)
    } else {
        Err(gl.get_shader_info_log(&shader).unwrap_or_default().into())
    }
}

fn link(gl: &Gl, vertex: &str, fragment: &str) -> Result<WebGlProgram, JsValue> {
    let vertex = compile(gl, Gl::VERTEX_SHADER, vertex)?;
    let fragment = compile(gl, Gl::FRAGMENT_SHADER, fragment)?;
    let program = gl.create_program().ok_or("create_program")?;
    gl.attach_shader(&program, &vertex);
    gl.attach_shader(&program, &fragment);
    gl.link_program(&program);
    if gl
        .get_program_parameter(&program, Gl::LINK_STATUS)
        .as_bool()
        .unwrap_or(false)
    {
        Ok(program)
    } else {
        Err(gl.get_program_info_log(&program).unwrap_or_default().into())
    }
}
