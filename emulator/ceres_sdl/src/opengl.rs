use glutin::{
    config::Config,
    display::GetGlDisplay,
    prelude::GlDisplay,
    surface::{Surface, SurfaceAttributesBuilder, WindowSurface},
};
use raw_window_handle::HasRawWindowHandle;
use std::{ffi::CString, num::NonZeroU32};
use winit::window::Window;
use {
    glow::{Context, HasContext, NativeProgram, NativeTexture, NativeVertexArray, UniformLocation},
    std::cmp::min,
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub struct Renderer {
    // Glow
    gl: Context,
    program: NativeProgram,
    _vao: NativeVertexArray,
    _texture: NativeTexture,
    uniform_loc: UniformLocation,
}

impl Renderer {
    pub fn new<D: GlDisplay>(gl_display: &D) -> Self {
        unsafe {
            let gl = glow::Context::from_loader_function(|symbol| {
                let symbol = CString::new(symbol).unwrap();
                gl_display.get_proc_address(symbol.as_c_str()).cast()
            });

            // create vao
            let vao = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));

            // create program
            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("../shader/vs.vert")),
                (glow::FRAGMENT_SHADER, include_str!("../shader/fs.frag")),
            ];

            let mut shaders = Vec::with_capacity(shader_sources.len());

            for (shader_type, shader_source) in &shader_sources {
                let shader = gl
                    .create_shader(*shader_type)
                    .expect("Cannot create shader");
                gl.shader_source(shader, shader_source);
                gl.compile_shader(shader);
                if !gl.get_shader_compile_status(shader) {
                    panic!("{}", gl.get_shader_info_log(shader));
                }
                gl.attach_shader(program, shader);
                shaders.push(shader);
            }

            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("{}", gl.get_program_info_log(program));
            }

            for shader in shaders {
                gl.detach_shader(program, shader);
                gl.delete_shader(shader);
            }

            gl.use_program(Some(program));

            // create texture
            let texture = gl.create_texture().expect("cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );

            let uniform_loc = gl
                .get_uniform_location(program, "transform")
                .expect("couldn't get location of uniform");

            gl.clear_color(0.0, 0.0, 0.0, 1.0);

            let res = Self {
                gl,
                program,
                _vao: vao,
                _texture: texture,
                uniform_loc,
            };

            res.resize(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            res
        }
    }

    pub fn resize(&self, width: u32, height: u32) {
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);

            self.gl.use_program(Some(self.program));

            let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
            let x = (PX_WIDTH * mul) as f32 / width as f32;
            let y = (PX_HEIGHT * mul) as f32 / height as f32;

            self.gl.uniform_2_f32(Some(&self.uniform_loc), x, y);
        }
    }

    pub fn draw_frame(&self, rgb: &[u8]) {
        unsafe {
            // TODO: texture streaming
            //self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                Some(rgb),
            );

            self.gl.clear(glow::COLOR_BUFFER_BIT);
            //self.gl.use_program(Some(self.program));
            //self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
}

pub struct GlWindow {
    // XXX the surface must be dropped before the window.
    pub surface: Surface<WindowSurface>,

    pub window: Window,
}

impl GlWindow {
    pub fn new(window: Window, config: &Config) -> Self {
        let (width, height): (u32, u32) = window.inner_size().into();
        let raw_window_handle = window.raw_window_handle();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        );

        let surface = unsafe {
            config
                .display()
                .create_window_surface(config, &attrs)
                .unwrap()
        };

        Self { surface, window }
    }
}
