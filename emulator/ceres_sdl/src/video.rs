use {
    crate::CERES_STYLIZED,
    glow::{Context, HasContext, NativeProgram, NativeTexture, NativeVertexArray, UniformLocation},
    glutin::{
        dpi::PhysicalSize,
        event_loop::EventLoop,
        window::{Fullscreen, WindowBuilder},
        ContextBuilder, GlProfile, GlRequest, PossiblyCurrent, Robustness, WindowedContext,
    },
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

    // Context
    ctx_wrapper: WindowedContext<PossiblyCurrent>,
}

impl Renderer {
    pub fn new(events: &EventLoop<()>) -> Self {
        unsafe {
            let window_builder = WindowBuilder::new()
                .with_title(CERES_STYLIZED)
                .with_inner_size(PhysicalSize {
                    width: PX_WIDTH as i32 * 4,
                    height: PX_HEIGHT as i32 * 4,
                })
                .with_min_inner_size(PhysicalSize {
                    width: PX_WIDTH as i32,
                    height: PX_HEIGHT as i32,
                });

            let ctx_wrapper = ContextBuilder::new()
                .with_gl(GlRequest::Latest)
                .with_gl_profile(GlProfile::Core)
                .with_gl_robustness(Robustness::NotRobust)
                .with_vsync(true)
                .build_windowed(window_builder, events)
                .unwrap()
                .make_current()
                .unwrap();

            let gl =
                glow::Context::from_loader_function(|s| ctx_wrapper.get_proc_address(s).cast());

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
                ctx_wrapper,
            };

            res.resize(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            res
        }
    }

    pub fn toggle_fullscreen(&mut self) {
        let in_fullscreen = self.ctx_wrapper.window().fullscreen();

        match in_fullscreen {
            Some(_) => self.ctx_wrapper.window().set_fullscreen(None),
            None => self
                .ctx_wrapper
                .window()
                .set_fullscreen(Some(Fullscreen::Borderless(None))),
        }

        let size = self.ctx_wrapper.window().inner_size();
        self.resize(size.width, size.height);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.ctx_wrapper.resize(PhysicalSize { width, height });

            self.gl.use_program(Some(self.program));

            let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
            let x = (PX_WIDTH * mul) as f32 / width as f32;
            let y = (PX_HEIGHT * mul) as f32 / height as f32;

            self.gl.uniform_2_f32(Some(&self.uniform_loc), x, y);
        }
    }

    pub fn draw_frame(&mut self, rgb: &[u8]) {
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

            self.ctx_wrapper.swap_buffers().unwrap();
        }
    }
}
