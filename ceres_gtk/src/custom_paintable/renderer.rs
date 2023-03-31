use {
    glow::{Context, HasContext, NativeProgram, NativeTexture, NativeVertexArray, UniformLocation},
    std::cmp::min,
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

pub struct Renderer {
    gl: Context,
    program: NativeProgram,
    vao: NativeVertexArray,
    texture: NativeTexture,
    uniform_loc: UniformLocation,
}

impl Renderer {
    #[allow(clippy::too_many_lines)]
    pub fn new() -> Self {
        unsafe {
            let gl = glow::Context::from_loader_function(epoxy::get_proc_addr);

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

            Self {
                gl,
                program,
                vao,
                texture,
                uniform_loc,
            }
        }
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
        let img_w = PX_WIDTH * mul;
        let img_h = PX_HEIGHT * mul;
        let uniform_x = img_w as f32 / width as f32;
        let uniform_y = img_h as f32 / height as f32;

        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);
            self.gl.use_program(Some(self.program));
            self.gl
                .uniform_2_f32(Some(&self.uniform_loc), uniform_x, uniform_y);
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
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
