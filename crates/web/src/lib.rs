//! The browser-facing half: wasm-bindgen exports and the WebGL2 renderer.
//! The page (`web/`) owns all UI; this crate owns the canvas.

use wasm_bindgen::prelude::*;
use web_sys::{HtmlCanvasElement, WebGl2RenderingContext as Gl, WebGlProgram, WebGlShader};

const VERTEX_SHADER: &str = "#version 300 es
in vec2 position;
in vec3 color;
out vec3 v_color;
void main() {
    v_color = color;
    gl_Position = vec4(position, 0.0, 1.0);
}";

const FRAGMENT_SHADER: &str = "#version 300 es
precision mediump float;
in vec3 v_color;
out vec4 frag;
void main() {
    frag = vec4(v_color, 1.0);
}";

#[wasm_bindgen(start)]
fn start() {
    console_error_panic_hook::set_once();
}

/// The core crate's version.
#[wasm_bindgen]
pub fn version() -> String {
    historyflow_core::version().to_string()
}

/// Owns the WebGL2 context of one canvas.
#[wasm_bindgen]
pub struct Renderer {
    gl: Gl,
    program: WebGlProgram,
}

#[wasm_bindgen]
impl Renderer {
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: &HtmlCanvasElement) -> Result<Renderer, JsValue> {
        let gl: Gl = canvas
            .get_context("webgl2")?
            .ok_or("WebGL2 is not available")?
            .dyn_into()?;
        let program = link(&gl)?;
        Ok(Renderer { gl, program })
    }

    /// Draws the placeholder scene: two flat-colored bands on black. It
    /// exists to prove the build → wasm → GL → pixels path end to end and
    /// gives the e2e golden something deterministic to pin.
    pub fn draw(&self) -> Result<(), JsValue> {
        let gl = &self.gl;
        gl.viewport(0, 0, gl.drawing_buffer_width(), gl.drawing_buffer_height());
        gl.clear_color(0.0, 0.0, 0.0, 1.0);
        gl.clear(Gl::COLOR_BUFFER_BIT);
        gl.use_program(Some(&self.program));

        // x, y, r, g, b per vertex; two quads as triangles.
        #[rustfmt::skip]
        let vertices: [f32; 60] = [
            -0.8,  0.6, 0.90, 0.45, 0.10,   0.8,  0.2, 0.90, 0.45, 0.10,   0.8,  0.6, 0.90, 0.45, 0.10,
            -0.8,  0.6, 0.90, 0.45, 0.10,  -0.8,  0.1, 0.90, 0.45, 0.10,   0.8,  0.2, 0.90, 0.45, 0.10,
            -0.8,  0.1, 0.15, 0.55, 0.85,   0.8, -0.6, 0.15, 0.55, 0.85,   0.8,  0.2, 0.15, 0.55, 0.85,
            -0.8,  0.1, 0.15, 0.55, 0.85,  -0.8, -0.3, 0.15, 0.55, 0.85,   0.8, -0.6, 0.15, 0.55, 0.85,
        ];
        let vao = gl.create_vertex_array().ok_or("create_vertex_array")?;
        gl.bind_vertex_array(Some(&vao));
        let buffer = gl.create_buffer().ok_or("create_buffer")?;
        gl.bind_buffer(Gl::ARRAY_BUFFER, Some(&buffer));
        let data = js_sys::Float32Array::from(vertices.as_slice());
        gl.buffer_data_with_array_buffer_view(Gl::ARRAY_BUFFER, &data, Gl::STATIC_DRAW);

        let stride = 5 * 4;
        let position = gl.get_attrib_location(&self.program, "position") as u32;
        gl.enable_vertex_attrib_array(position);
        gl.vertex_attrib_pointer_with_i32(position, 2, Gl::FLOAT, false, stride, 0);
        let color = gl.get_attrib_location(&self.program, "color") as u32;
        gl.enable_vertex_attrib_array(color);
        gl.vertex_attrib_pointer_with_i32(color, 3, Gl::FLOAT, false, stride, 2 * 4);

        gl.draw_arrays(Gl::TRIANGLES, 0, 12);
        gl.delete_buffer(Some(&buffer));
        gl.delete_vertex_array(Some(&vao));
        Ok(())
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

fn link(gl: &Gl) -> Result<WebGlProgram, JsValue> {
    let vertex = compile(gl, Gl::VERTEX_SHADER, VERTEX_SHADER)?;
    let fragment = compile(gl, Gl::FRAGMENT_SHADER, FRAGMENT_SHADER)?;
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
