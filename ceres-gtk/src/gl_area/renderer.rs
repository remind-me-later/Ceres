use glow::{Context, HasContext, NativeProgram, NativeTexture, NativeVertexArray, UniformLocation};

#[derive(Clone, Copy, Default)]
pub enum PxScaleMode {
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
    Lcd = 3,
    Crt = 4,
}

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;

pub struct Renderer {
    gl: Context,
    program: NativeProgram,
    vao: NativeVertexArray,
    // pbo: glow::Buffer,
    texture: NativeTexture,
    texture_back: NativeTexture,
    dims_unif: UniformLocation,
    scale_unif: UniformLocation,
    new_size: Option<(u32, u32)>,
    new_scale_mode: Option<PxScaleMode>,
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
                (
                    glow::VERTEX_SHADER,
                    include_str!("../../shader/shader.vert"),
                ),
                (
                    glow::FRAGMENT_SHADER,
                    include_str!("../../shader/shader.frag"),
                ),
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
            gl.active_texture(glow::TEXTURE0);
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
            let main_texture_unif = gl.get_uniform_location(program, "_group_0_binding_0_fs").expect("couldn't get location of main texture uniform");
            gl.uniform_1_i32(Some(&main_texture_unif), 0);


            let texture_back = gl.create_texture().expect("cannot create texture");
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_back));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );
            let back_texture_unif = gl.get_uniform_location(program, "_group_0_binding_2_fs").expect("couldn't get location of back texture uniform");
            gl.uniform_1_i32(Some(&back_texture_unif), 2);


            let dims_unif = gl
                .get_uniform_location(program, "_group_1_binding_0_vs")
                .expect("couldn't get location of dimensions uniform");

            let scale_unif = gl
                .get_uniform_location(program, "_group_1_binding_1_fs")
                .expect("couldn't get location of scale uniform");

            // Init scale uniform
            gl.uniform_1_u32_slice(Some(&scale_unif), &[PxScaleMode::Nearest as u32]);

            // let pbo = gl.create_buffer().expect("cannot create pbo");
            // gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(pbo));
            // gl.buffer_data_size(
            //     glow::PIXEL_UNPACK_BUFFER,
            //     (PX_WIDTH * PX_HEIGHT * 3) as i32,
            //     glow::STREAM_DRAW,
            // );

            Self {
                gl,
                program,
                vao,
                // pbo,
                texture,
                texture_back,
                dims_unif,
                scale_unif,
                new_size: None,
                new_scale_mode: None,
            }
        }
    }

    pub fn choose_scale_mode(&mut self, scale_mode: PxScaleMode) {
        self.new_scale_mode = Some(scale_mode);
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        self.new_size = Some((width, height));
    }

    pub fn draw_frame(&mut self, rgba: &[u8]) {
        unsafe {
            // self.gl
            //     .bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(self.pbo));

            // #[allow(clippy::cast_possible_truncation)]
            // let buf = self.gl.map_buffer_range(
            //     glow::PIXEL_UNPACK_BUFFER,
            //     0,
            //     rgb.len() as i32,
            //     glow::MAP_WRITE_BIT | glow::MAP_INVALIDATE_BUFFER_BIT,
            // );

            // std::ptr::copy_nonoverlapping(rgb.as_ptr(), buf, rgb.len());

            // self.gl.unmap_buffer(glow::PIXEL_UNPACK_BUFFER);

            self.gl.active_texture(glow::TEXTURE0);
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
                glow::PixelUnpackData::Slice(Some(rgba)),
            );

            self.gl.active_texture(glow::TEXTURE2);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.texture_back));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(rgba)),
            );

            self.gl.use_program(Some(self.program));

            #[allow(clippy::cast_precision_loss)]
            if let Some((width, height)) = self.new_size.take() {
                // resize image to fit the window
                let mul = (width as f32 / PX_WIDTH as f32).min(height as f32 / PX_HEIGHT as f32);
                let img_w = PX_WIDTH as f32 * mul;
                let img_h = PX_HEIGHT as f32 * mul;
                let uniform_x = img_w as f32 / width as f32;
                let uniform_y = img_h as f32 / height as f32;

                self.gl.viewport(0, 0, width as i32, height as i32);
                self.gl
                    .uniform_2_f32(Some(&self.dims_unif), uniform_x, uniform_y);
            }

            if let Some(scale_mode) = self.new_scale_mode.take() {
                // set scaling mode
                self.gl
                    .uniform_1_u32_slice(Some(&self.scale_unif), &[scale_mode as u32]);
            }

            self.gl.bind_vertex_array(Some(self.vao));

            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
