use {
    glow::{Context, HasContext, NativeProgram, NativeTexture, NativeVertexArray, UniformLocation},
    sdl2::{
        video::{FullscreenType, GLContext, SwapInterval, Window},
        Sdl, VideoSubsystem,
    },
    std::{cmp::min, mem::ManuallyDrop},
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub struct Renderer {
    // Glow
    gl: Context,
    program: NativeProgram,
    vao: NativeVertexArray,
    texture: NativeTexture,
    uniform_loc: UniformLocation,

    // SDL
    win: Window,
    video: ManuallyDrop<VideoSubsystem>,
    ctx: ManuallyDrop<GLContext>,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        unsafe {
            let video = ManuallyDrop::new(sdl.video().unwrap());

            let gl_attr = video.gl_attr();
            gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
            gl_attr.set_context_version(4, 6);
            gl_attr.set_depth_size(0);
            gl_attr.set_context_flags().forward_compatible().set();

            if !cfg!(debug_assertions) {
                gl_attr.set_context_no_error(true);
            }

            let mut win = video
                .window(crate::CERES_STR, PX_WIDTH * MUL, PX_HEIGHT * MUL)
                .opengl()
                .position_centered()
                .resizable()
                .build()
                .unwrap();
            win.set_minimum_size(PX_WIDTH, PX_HEIGHT).unwrap();

            let ctx = ManuallyDrop::new(win.gl_create_context().unwrap());
            win.gl_make_current(&ctx).unwrap();
            video.gl_set_swap_interval(SwapInterval::VSync).unwrap();

            let gl = glow::Context::from_loader_function(|s| video.gl_get_proc_address(s).cast());

            // create vao
            let vao = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));

            // create program
            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("shader/vs.vert")),
                (glow::FRAGMENT_SHADER, include_str!("shader/fs.frag")),
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

            let mut res = Self {
                gl,
                program,
                vao,
                texture,
                uniform_loc,
                win,
                video,
                ctx,
            };

            res.resize(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            res
        }
    }

    pub fn toggle_fullscreen(&mut self) {
        let in_fullscreen = self.win.fullscreen_state();

        match in_fullscreen {
            FullscreenType::Off => self.win.set_fullscreen(FullscreenType::Desktop).unwrap(),
            _ => self.win.set_fullscreen(FullscreenType::Off).unwrap(),
        }

        let (w, h) = self.win.size();
        self.resize(w, h);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.win.set_size(width, height).unwrap();

            self.gl.use_program(Some(self.program));

            let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
            let x = (PX_WIDTH * mul) as f32 / width as f32;
            let y = (PX_HEIGHT * mul) as f32 / height as f32;

            self.gl.uniform_2_f32(Some(&self.uniform_loc), x, y);
        }
    }

    pub fn draw_frame(&mut self, rgba: &[u8]) {
        unsafe {
            // TODO: texture streaming
            self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                Some(rgba),
            );

            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.use_program(Some(self.program));
            self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }

        self.win.gl_swap_window();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            ManuallyDrop::drop(&mut self.ctx);
            ManuallyDrop::drop(&mut self.video);
        }
    }
}
