use sdl2::{
    video::{GLContext, GLProfile, SwapInterval, Window},
    Sdl,
};

use crate::CERES_STYLIZED;

use {glow::HasContext, std::cmp::min};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub struct Renderer {
    gl: glow::Context,
    program: glow::NativeProgram,
    _vao: glow::NativeVertexArray,
    _texture: glow::NativeTexture,
    uniform_loc: glow::UniformLocation,
    pixel_perfect: bool,

    sdl_window: Window,
    _sdl_ctx: GLContext,
}

impl Renderer {
    pub fn new(sdl_context: &Sdl) -> Self {
        let video_subsystem = sdl_context.video().unwrap();

        let mut sdl_window = video_subsystem
            .window(
                CERES_STYLIZED,
                ceres_core::PX_WIDTH as u32 * 4,
                ceres_core::PX_HEIGHT as u32 * 4,
            )
            .resizable()
            .opengl()
            .build()
            .unwrap();

        sdl_window
            .set_minimum_size(ceres_core::PX_WIDTH as u32, ceres_core::PX_HEIGHT as u32)
            .unwrap();

        let gl_attr = video_subsystem.gl_attr();
        gl_attr.set_context_profile(GLProfile::Core);
        gl_attr.set_context_version(4, 6);

        // Unlike the other example above, nobody created a context for your window, so you need to create one.
        let sdl_ctx = sdl_window.gl_create_context().unwrap();

        debug_assert_eq!(gl_attr.context_profile(), GLProfile::Core);
        debug_assert_eq!(gl_attr.context_version(), (4, 6));

        video_subsystem
            .gl_set_swap_interval(SwapInterval::VSync)
            .unwrap();

        unsafe {
            let gl = glow::Context::from_loader_function(|symbol| {
                video_subsystem.gl_get_proc_address(symbol).cast()
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

            let mut res = Self {
                gl,
                program,
                _vao: vao,
                _texture: texture,
                uniform_loc,
                pixel_perfect: false,
                sdl_window,
                _sdl_ctx: sdl_ctx,
            };

            res.resize(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            res
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);

            self.gl.use_program(Some(self.program));

            let (x, y) = if self.pixel_perfect {
                let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
                let x = (PX_WIDTH * mul) as f32 / width as f32;
                let y = (PX_HEIGHT * mul) as f32 / height as f32;
                (x, y)
            } else {
                let l = width as f32 / PX_WIDTH as f32;
                let r = height as f32 / PX_HEIGHT as f32;
                let mul = if l < r { l } else { r };
                let x = (PX_WIDTH as f32 * mul) / width as f32;
                let y = (PX_HEIGHT as f32 * mul) / height as f32;
                (x, y)
            };

            self.gl.uniform_2_f32(Some(&self.uniform_loc), x, y);
        }
    }

    pub fn render(&mut self, rgb: &[u8]) {
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

            self.sdl_window.gl_swap_window();
        }
    }
}
