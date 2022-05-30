use {
    core::{ffi::c_void, mem, ptr},
    gl::types::*,
    sdl2::{
        video::{GLContext, Window},
        Sdl, VideoSubsystem,
    },
    std::time::Instant,
};

const INDICES: [GLubyte; 6] = [
    0, 1, 3, // first triangle
    1, 2, 3, // second triangle
];

const VERTICES: [GLfloat; 16] = [
    // positions  // texture coords
    1.0, 1.0, 1.0, 0.0, // top right
    1.0, -1.0, 1.0, 1.0, // bottom right
    -1.0, -1.0, 0.0, 1.0, // bottom left
    -1.0, 1.0, 0.0, 0.0, // top left
];

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub trait Context {
    fn get_proc_address(&mut self, procname: &str) -> *const c_void;
    fn swap_buffers(&mut self);
    fn make_current(&mut self);
}

pub struct Renderer {
    window: Window,
    next_frame: Instant,
    vbo: GLuint,
    vao: GLuint,
    ebo: GLuint,
    program: Program,
    texture: GLuint,
    transform_location: GLint,
    // keep them safe and cozy
    _video_subsystem: VideoSubsystem,
    _context: GLContext,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Renderer {
        unsafe {
            let video_subsystem = sdl.video().unwrap();

            let gl_attr = video_subsystem.gl_attr();
            gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
            gl_attr.set_context_version(3, 3);

            let mut window = video_subsystem
                .window(crate::CERES_STR, PX_WIDTH * MUL, PX_HEIGHT * MUL)
                .opengl()
                .position_centered()
                .resizable()
                .build()
                .unwrap();

            window.set_minimum_size(PX_WIDTH, PX_HEIGHT).unwrap();

            let context = window.gl_create_context().unwrap();
            window.gl_make_current(&context).unwrap();

            gl::load_with(|s| video_subsystem.gl_get_proc_address(s).cast());

            let program = {
                let vert_shader =
                    Shader::from_src(include_bytes!("shader/vs.vert"), gl::VERTEX_SHADER);
                let frag_shader =
                    Shader::from_src(include_bytes!("shader/fs.frag"), gl::FRAGMENT_SHADER);
                Program::new(vert_shader, frag_shader)
            };

            let mut vbo = 0;
            let mut vao = 0;
            let mut ebo = 0;

            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
            gl::GenBuffers(1, &mut ebo);

            gl::BindVertexArray(vao);

            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (VERTICES.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                mem::transmute(&VERTICES[0]),
                gl::STATIC_DRAW,
            );

            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (INDICES.len() * mem::size_of::<GLubyte>()) as GLsizeiptr,
                mem::transmute(&INDICES[0]),
                gl::STATIC_DRAW,
            );

            let stride = (4 * mem::size_of::<GLfloat>()) as GLsizei;

            // position attribute
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, stride, ptr::null());
            gl::EnableVertexAttribArray(0);

            // texture coordinates attribute
            gl::VertexAttribPointer(
                1,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * mem::size_of::<GLfloat>()) as *const c_void,
            );
            gl::EnableVertexAttribArray(1);

            // create texture
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            // scaling behaviour
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);

            // create transform
            let transform_name = b"transform\0";
            let transform_location = program.get_uniform_location(transform_name.as_ptr().cast());

            let mut video_renderer = Renderer {
                window,
                vbo,
                vao,
                ebo,
                program,
                texture,
                transform_location,
                next_frame: Instant::now(),
                _video_subsystem: video_subsystem,
                _context: context,
            };

            video_renderer.resize_viewport(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            video_renderer
        }
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let gb_width = ceres_core::PX_WIDTH as u32;
        let gb_height = ceres_core::PX_HEIGHT as u32;
        let multiplier = core::cmp::min(width / gb_width, height / gb_height);
        let surface_width = gb_width * multiplier;
        let surface_height = gb_height * multiplier;

        let x = surface_width as f32 / width as f32;
        let y = surface_height as f32 / height as f32;

        // hand-written scale matrix
        let transform = [
            x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        unsafe {
            self.program.use_program();
            gl::Viewport(0, 0, width as i32, height as i32);
            gl::UniformMatrix4fv(self.transform_location, 1, gl::FALSE, transform.as_ptr());
        }
    }

    unsafe fn update_texture(&mut self, rgba: &[u8]) {
        // TODO: texture streaming
        gl::BindTexture(gl::TEXTURE_2D, self.texture);
        gl::TexImage2D(
            gl::TEXTURE_2D,
            0,
            gl::RGBA as GLint,
            ceres_core::PX_WIDTH as GLint,
            ceres_core::PX_HEIGHT as GLint,
            0,
            gl::RGBA,
            gl::UNSIGNED_BYTE,
            rgba.as_ptr().cast(),
        );
    }

    pub fn draw_frame(&mut self, rgba_pixel_data: &[u8]) {
        unsafe {
            self.update_texture(rgba_pixel_data);

            let now = Instant::now();
            if now < self.next_frame {
                std::thread::sleep(self.next_frame - now);
            }

            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            self.program.use_program();
            gl::BindVertexArray(self.vao);
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_BYTE, ptr::null());

            self.window.gl_swap_window();

            self.next_frame += ceres_core::FRAME_DUR;
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

struct Shader {
    id: GLuint,
}

impl Shader {
    pub unsafe fn from_src(source: &'static [u8], shader_type: GLenum) -> Self {
        let id = gl::CreateShader(shader_type);
        // Attempt to compile the shader
        let mut c_string = source.to_vec();
        c_string.push(b'\0');
        gl::ShaderSource(id, 1, &(c_string.as_ptr().cast()), ptr::null());
        gl::CompileShader(id);

        // Get the compile status
        let mut status = gl::FALSE as GLint;
        gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut status);

        if status != gl::TRUE as GLint {
            let mut len = 0;
            gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            gl::GetShaderInfoLog(id, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
            buf.pop(); // ignore '/0'

            let msg = core::str::from_utf8(&buf)
                .unwrap_or("ShaderInfoLog not valid utf8")
                .to_owned();

            panic!("{msg}");
        }

        Self { id }
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}

struct Program {
    id: GLuint,
    // keep them alive
    _vert_shader: Shader,
    _frag_shader: Shader,
}

impl Program {
    pub unsafe fn new(vert_shader: Shader, frag_shader: Shader) -> Self {
        let id = gl::CreateProgram();
        gl::AttachShader(id, vert_shader.id);
        gl::AttachShader(id, frag_shader.id);
        gl::LinkProgram(id);
        // Get the link status
        let mut status = gl::FALSE as GLint;
        gl::GetProgramiv(id, gl::LINK_STATUS, &mut status);

        if status != gl::TRUE as GLint {
            let mut len: GLint = 0;
            gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut len);
            let mut buf = Vec::with_capacity(len as usize);
            gl::GetProgramInfoLog(id, len, ptr::null_mut(), buf.as_mut_ptr() as *mut GLchar);
            buf.pop(); // ignore '/0'

            let msg = core::str::from_utf8(&buf)
                .unwrap_or("ShaderInfoLog not valid utf8")
                .to_owned();

            panic!("{msg}");
        }

        Self {
            id,
            _vert_shader: vert_shader,
            _frag_shader: frag_shader,
        }
    }

    pub unsafe fn use_program(&self) {
        gl::UseProgram(self.id);
    }

    pub unsafe fn get_uniform_location(&self, name: *const GLchar) -> GLint {
        gl::GetUniformLocation(self.id, name)
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
