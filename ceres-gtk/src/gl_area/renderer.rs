use glow::{
    Context, HasContext, NativeBuffer, NativeProgram, NativeTexture, NativeVertexArray,
    UniformLocation,
};

const PBO_BUFFER_SIZE: i32 = (PX_WIDTH * PX_HEIGHT * 4) as i32;
const PX_HEIGHT: u32 = ceres_std::PX_HEIGHT as u32;
const PX_WIDTH: u32 = ceres_std::PX_WIDTH as u32;
const INITIAL_TEXTURE_DATA_SIZE: usize = (PX_WIDTH * PX_HEIGHT * 4) as usize;

#[derive(Clone, Copy, Default)]
pub enum ShaderMode {
    Crt = 4,
    Lcd = 3,
    #[default]
    Nearest = 0,
    Scale2x = 1,
    Scale3x = 2,
}

impl From<&str> for ShaderMode {
    fn from(s: &str) -> Self {
        match s {
            "Nearest" => Self::Nearest,
            "Scale2x" => Self::Scale2x,
            "Scale3x" => Self::Scale3x,
            "LCD" => Self::Lcd,
            "CRT" => Self::Crt,
            _ => Self::default(),
        }
    }
}

impl std::fmt::Display for ShaderMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Self::Nearest => "Nearest",
            Self::Scale2x => "Scale2x",
            Self::Scale3x => "Scale3x",
            Self::Lcd => "LCD",
            Self::Crt => "CRT",
        };
        write!(f, "{name}")
    }
}

pub struct Renderer {
    dims_unif: UniformLocation,
    gl: Context,
    new_scale_mode: Option<ShaderMode>,
    new_size: Option<(u32, u32)>,
    pbo_upload_current: NativeBuffer,
    program: NativeProgram,
    scale_unif: UniformLocation,
    texture_current: NativeTexture,
    texture_previous: NativeTexture,
    vao: NativeVertexArray,
}

impl Renderer {
    pub const fn choose_scale_mode(&mut self, scale_mode: ShaderMode) {
        self.new_scale_mode = Some(scale_mode);
    }

    pub fn draw_frame(&mut self, rgba: &[u8]) {
        unsafe {
            // Copy current texture to previous texture
            self.gl.copy_image_sub_data(
                self.texture_current,
                glow::TEXTURE_2D,
                0,
                0,
                0,
                0,
                self.texture_previous,
                glow::TEXTURE_2D,
                0,
                0,
                0,
                0,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                1,
            );

            // Upload new rgba data to current texture via PBO
            self.gl
                .bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(self.pbo_upload_current));
            let ptr = self.gl.map_buffer_range(
                glow::PIXEL_UNPACK_BUFFER,
                0,
                PBO_BUFFER_SIZE,
                glow::MAP_WRITE_BIT | glow::MAP_INVALIDATE_BUFFER_BIT,
            );
            if !ptr.is_null() {
                let dest_slice =
                    core::slice::from_raw_parts_mut(ptr.cast::<u8>(), PBO_BUFFER_SIZE as usize);
                dest_slice.copy_from_slice(rgba);
                self.gl.unmap_buffer(glow::PIXEL_UNPACK_BUFFER);
            } else {
                eprintln!("Failed to map PBO for current texture");
            }

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.texture_current)); // Ensure correct texture is bound
            self.gl.tex_sub_image_2d(
                glow::TEXTURE_2D,
                0,
                0,
                0,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::BufferOffset(0),
            );
            self.gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);

            // Ensure previous texture is active on its unit (though binding persists, good for clarity)
            self.gl.active_texture(glow::TEXTURE2);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.texture_previous));

            self.gl.use_program(Some(self.program));

            if let Some((width, height)) = self.new_size.take() {
                // resize image to fit the window
                let mul = (width as f32 / PX_WIDTH as f32).min(height as f32 / PX_HEIGHT as f32);
                let img_w = PX_WIDTH as f32 * mul;
                let img_h = PX_HEIGHT as f32 * mul;
                let uniform_x = img_w / width as f32;
                let uniform_y = img_h / height as f32;

                self.gl.viewport(0, 0, width as i32, height as i32);
                self.gl
                    .uniform_2_f32(Some(&self.dims_unif), uniform_x, uniform_y);
            }

            if let Some(scale_mode) = self.new_scale_mode.take() {
                self.gl
                    .uniform_1_u32_slice(Some(&self.scale_unif), &[scale_mode as u32]);
            }

            self.gl.bind_vertex_array(Some(self.vao));

            self.gl.clear_color(0.0, 0.0, 0.0, 1.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Initialization logic is long but necessary"
    )]
    pub fn new() -> Self {
        unsafe {
            let gl = glow::Context::from_loader_function(epoxy::get_proc_addr);

            let vao = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));

            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (
                    glow::VERTEX_SHADER,
                    include_str!("../../shader/vshader.vert"),
                ),
                (
                    glow::FRAGMENT_SHADER,
                    include_str!("../../shader/fshader.frag"),
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

            let initial_pixel_data: Vec<u8> = vec![0; INITIAL_TEXTURE_DATA_SIZE];

            let texture_current = gl.create_texture().expect("cannot create current texture");
            gl.active_texture(glow::TEXTURE0);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_current));
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
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&initial_pixel_data)),
            );
            let current_texture_unif = gl
                .get_uniform_location(program, "_group_0_binding_0_fs")
                .expect("couldn't get location of current texture uniform");
            gl.uniform_1_i32(Some(&current_texture_unif), 0);

            let texture_previous = gl.create_texture().expect("cannot create previous texture");
            gl.active_texture(glow::TEXTURE2);
            gl.bind_texture(glow::TEXTURE_2D, Some(texture_previous));
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
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(&initial_pixel_data)),
            );
            let previous_texture_unif = gl
                .get_uniform_location(program, "_group_0_binding_2_fs")
                .expect("couldn't get location of previous texture uniform");
            gl.uniform_1_i32(Some(&previous_texture_unif), 2);

            let pbo_upload_current = gl
                .create_buffer()
                .expect("cannot create PBO for current texture");
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(pbo_upload_current));
            gl.buffer_data_size(
                glow::PIXEL_UNPACK_BUFFER,
                PBO_BUFFER_SIZE,
                glow::STREAM_DRAW,
            );
            gl.bind_buffer(glow::PIXEL_UNPACK_BUFFER, None);

            let dims_unif = gl
                .get_uniform_location(program, "_group_1_binding_0_vs")
                .expect("couldn't get location of dimensions uniform");

            let scale_unif = gl
                .get_uniform_location(program, "_group_1_binding_1_fs")
                .expect("couldn't get location of scale uniform");

            gl.uniform_1_u32_slice(Some(&scale_unif), &[ShaderMode::Nearest as u32]);

            Self {
                gl,
                program,
                vao,
                texture_current,
                texture_previous,
                pbo_upload_current,
                dims_unif,
                scale_unif,
                new_size: None,
                new_scale_mode: None,
            }
        }
    }

    pub const fn resize_viewport(&mut self, width: u32, height: u32) {
        self.new_size = Some((width, height));
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
