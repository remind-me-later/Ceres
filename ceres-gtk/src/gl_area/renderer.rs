use glow::{Context, HasContext, NativeBuffer, NativeProgram, NativeTexture, NativeVertexArray};

const PBO_BUFFER_SIZE: i32 = (PX_WIDTH * PX_HEIGHT * 4) as i32;
const PX_HEIGHT: u32 = ceres_std::PX_HEIGHT as u32;
const PX_WIDTH: u32 = ceres_std::PX_WIDTH as u32;
const INITIAL_TEXTURE_DATA_SIZE: usize = (PX_WIDTH * PX_HEIGHT * 4) as usize;
/// The shader's resources (`build.rs` gives them these bindings): texture
/// units for the current and the previous frame, uniform buffer binding
/// points for the image's size in the viewport and the shader option.
const CURRENT_FRAME_UNIT: u32 = glow::TEXTURE0;
const PREVIOUS_FRAME_UNIT: u32 = glow::TEXTURE2;
const DIMENSIONS_BINDING: u32 = 0;
const SHADER_OPTION_BINDING: u32 = 1;
/// A uniform buffer's size: one std140 slot.
const UNIFORM_BUFFER_SIZE: i32 = 16;

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

/// Draws the Game Boy screen.
///
/// The current and the previous frame (the LCD shader blends the previous one
/// in) are two textures that swap roles on each new frame: a new frame is
/// uploaded over the older one.
pub struct Renderer {
    /// The texture holding the current frame.
    current: usize,
    current_size: (u32, u32),
    /// The uniform buffer of the image's size in the viewport.
    dims_buffer: NativeBuffer,
    gl: Context,
    new_scale_mode: Option<ShaderMode>,
    new_size: Option<(u32, u32, bool)>,
    pbo_upload_current: NativeBuffer,
    program: NativeProgram,
    /// The uniform buffer of the shader option.
    scale_buffer: NativeBuffer,
    textures: [NativeTexture; 2],
    vao: NativeVertexArray,
}

impl Renderer {
    pub const fn choose_scale_mode(&mut self, scale_mode: ShaderMode) {
        self.new_scale_mode = Some(scale_mode);
    }

    pub const fn current_size(&self) -> (u32, u32) {
        self.current_size
    }

    /// Draws the current frame (and the previous one, for the LCD shader).
    pub fn draw(&mut self) {
        unsafe {
            // The shader reads the current frame on unit 0, the previous one
            // on unit 2.
            self.gl.active_texture(CURRENT_FRAME_UNIT);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.textures[self.current]));
            self.gl.active_texture(PREVIOUS_FRAME_UNIT);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.textures[1 - self.current]));

            self.gl.use_program(Some(self.program));

            if let Some((width, height, use_pixel_perfect)) = self.new_size.take() {
                // resize image to fit the window
                let mul = if use_pixel_perfect {
                    (width / PX_WIDTH).min(height / PX_HEIGHT) as f32
                } else {
                    (width as f32 / PX_WIDTH as f32).min(height as f32 / PX_HEIGHT as f32)
                };

                let img_w = PX_WIDTH as f32 * mul;
                let img_h = PX_HEIGHT as f32 * mul;
                let uniform_x = img_w / width as f32;
                let uniform_y = img_h / height as f32;

                self.gl.viewport(0, 0, width as i32, height as i32);
                self.write_uniform(self.dims_buffer, &[uniform_x, uniform_y]);
            }

            if let Some(scale_mode) = self.new_scale_mode.take() {
                self.write_uniform(self.scale_buffer, &[scale_mode as u32]);
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
            let gl = glow::Context::from_loader_function(crate::epoxy::get_proc_address);

            let vao = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));

            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (
                    glow::VERTEX_SHADER,
                    include_str!(concat!(env!("OUT_DIR"), "/vshader.vert")),
                ),
                (
                    glow::FRAGMENT_SHADER,
                    include_str!(concat!(env!("OUT_DIR"), "/fshader.frag")),
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
            gl.active_texture(CURRENT_FRAME_UNIT);
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

            let texture_previous = gl.create_texture().expect("cannot create previous texture");
            gl.active_texture(PREVIOUS_FRAME_UNIT);
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

            let uniform_buffer = |binding| {
                let buffer = gl.create_buffer().expect("cannot create uniform buffer");
                gl.bind_buffer(glow::UNIFORM_BUFFER, Some(buffer));
                gl.buffer_data_size(
                    glow::UNIFORM_BUFFER,
                    UNIFORM_BUFFER_SIZE,
                    glow::DYNAMIC_DRAW,
                );
                gl.bind_buffer_base(glow::UNIFORM_BUFFER, binding, Some(buffer));
                buffer
            };
            let dims_buffer = uniform_buffer(DIMENSIONS_BINDING);
            let scale_buffer = uniform_buffer(SHADER_OPTION_BINDING);
            gl.bind_buffer(glow::UNIFORM_BUFFER, None);

            Self {
                current: 0,
                current_size: (PX_WIDTH, PX_HEIGHT),
                gl,
                program,
                vao,
                textures: [texture_current, texture_previous],
                pbo_upload_current,
                dims_buffer,
                scale_buffer,
                new_size: None,
                new_scale_mode: Some(ShaderMode::Nearest),
            }
        }
    }

    pub const fn resize_viewport(&mut self, width: u32, height: u32, pixel_perfect: bool) {
        self.new_size = Some((width, height, pixel_perfect));
        self.current_size = (width, height);
    }

    /// Uploads a new frame, which `write` puts in the pixel buffer: it goes
    /// over the older texture, and the current one becomes the previous one.
    pub fn upload_frame(&mut self, write: impl FnOnce(&mut [u8])) {
        self.current = 1 - self.current;
        unsafe {
            self.gl
                .bind_buffer(glow::PIXEL_UNPACK_BUFFER, Some(self.pbo_upload_current));
            // Invalidating lets the driver hand out fresh memory while the
            // last upload may still read the old one.
            let ptr = self.gl.map_buffer_range(
                glow::PIXEL_UNPACK_BUFFER,
                0,
                PBO_BUFFER_SIZE,
                glow::MAP_WRITE_BIT | glow::MAP_INVALIDATE_BUFFER_BIT,
            );
            if ptr.is_null() {
                eprintln!("Failed to map PBO for current texture");
            } else {
                write(core::slice::from_raw_parts_mut(
                    ptr.cast::<u8>(),
                    PBO_BUFFER_SIZE as usize,
                ));
                self.gl.unmap_buffer(glow::PIXEL_UNPACK_BUFFER);
            }

            self.gl.active_texture(glow::TEXTURE0);
            self.gl
                .bind_texture(glow::TEXTURE_2D, Some(self.textures[self.current]));
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
        }
    }

    /// Writes a uniform buffer's value.
    fn write_uniform<T: Copy>(&self, buffer: NativeBuffer, value: &[T]) {
        let bytes =
            unsafe { core::slice::from_raw_parts(value.as_ptr().cast::<u8>(), size_of_val(value)) };
        unsafe {
            self.gl.bind_buffer(glow::UNIFORM_BUFFER, Some(buffer));
            self.gl
                .buffer_sub_data_u8_slice(glow::UNIFORM_BUFFER, 0, bytes);
            self.gl.bind_buffer(glow::UNIFORM_BUFFER, None);
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
