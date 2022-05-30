use {
    gl::types::*,
    sdl2::{
        video::{GLContext, Window},
        Sdl, VideoSubsystem,
    },
    std::{
        cmp::min,
        ffi::{c_void, CString},
        mem::size_of,
        ptr,
        time::Instant,
    },
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

pub struct Renderer {
    window: Window,
    next_frame: Instant,
    vbo: GLuint,
    vao: GLuint,
    ebo: GLuint,
    program: GlProgram,
    texture: GLuint,
    transform_loc: GLint,
    // keep them safe and cozy
    _video: VideoSubsystem,
    _context: GLContext,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Renderer {
        unsafe {
            let video = sdl.video().unwrap();

            let gl_attr = video.gl_attr();
            gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
            gl_attr.set_context_version(3, 3);
            gl_attr.set_depth_size(0);
            gl_attr.set_context_flags().forward_compatible().set();

            if !cfg!(debug_assertions) {
                gl_attr.set_context_no_error(true);
            }

            let mut window = video
                .window(crate::CERES_STR, PX_WIDTH * MUL, PX_HEIGHT * MUL)
                .opengl()
                .position_centered()
                .resizable()
                .build()
                .unwrap();

            window.set_minimum_size(PX_WIDTH, PX_HEIGHT).unwrap();

            let context = window.gl_create_context().unwrap();
            window.gl_make_current(&context).unwrap();

            gl::load_with(|s| video.gl_get_proc_address(s).cast());

            let program = GlProgram::new(
                include_str!("shader/vs.vert"),
                include_str!("shader/fs.frag"),
            );

            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let mut vbo = 0;
            gl::GenBuffers(1, &mut vbo);
            gl::BindBuffer(gl::ARRAY_BUFFER, vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (VERTICES.len() * size_of::<GLfloat>()) as GLsizeiptr,
                VERTICES.as_ptr().cast(),
                gl::STATIC_DRAW,
            );

            let mut ebo = 0;
            gl::GenBuffers(1, &mut ebo);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, ebo);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (INDICES.len() * size_of::<GLubyte>()) as GLsizeiptr,
                INDICES.as_ptr().cast(),
                gl::STATIC_DRAW,
            );

            // position attribute
            let stride = (4 * size_of::<GLfloat>()) as GLsizei;
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, stride, ptr::null());
            gl::EnableVertexAttribArray(0);

            // texture coordinates attribute
            gl::VertexAttribPointer(
                1,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                (2 * size_of::<GLfloat>()) as *const c_void,
            );
            gl::EnableVertexAttribArray(1);

            // create texture
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            // scaling behaviour
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);

            // get transform pointer
            let transform_loc = program.get_uniform_location(b"transform\0".as_ptr().cast());

            let mut video_renderer = Renderer {
                window,
                vbo,
                vao,
                ebo,
                program,
                texture,
                transform_loc,
                next_frame: Instant::now(),
                _video: video,
                _context: context,
            };

            video_renderer.resize_viewport(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            video_renderer
        }
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let multiplier = min(width / PX_WIDTH, height / PX_HEIGHT);
        let surface_width = PX_WIDTH * multiplier;
        let surface_height = PX_HEIGHT * multiplier;

        let x = surface_width as f32 / width as f32;
        let y = surface_height as f32 / height as f32;

        // hand-written scale matrix
        let t = [
            x, 0.0, 0.0, 0.0, //
            0.0, y, 0.0, 0.0, //
            0.0, 0.0, 1.0, 0.0, //
            0.0, 0.0, 0.0, 1.0,
        ];

        unsafe {
            gl::Viewport(0, 0, width as i32, height as i32);
            self.program.bind();
            gl::UniformMatrix4fv(self.transform_loc, 1, gl::FALSE, t.as_ptr());
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

    pub fn draw_frame(&mut self, rgba: &[u8]) {
        unsafe {
            self.update_texture(rgba);

            let now = Instant::now();
            if now < self.next_frame {
                std::thread::sleep(self.next_frame - now);
            }

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            self.program.bind();
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

struct GlProgram {
    id: GLuint,
    vert_shader: GLuint,
    frag_shader: GLuint,
}

impl GlProgram {
    pub unsafe fn new(vert_src: &str, frag_src: &str) -> Self {
        let id = gl::CreateProgram();
        let vert_shader = Self::shader_from_src(vert_src, gl::VERTEX_SHADER);
        let frag_shader = Self::shader_from_src(frag_src, gl::FRAGMENT_SHADER);

        gl::AttachShader(id, vert_shader);
        gl::AttachShader(id, frag_shader);
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
            vert_shader,
            frag_shader,
        }
    }

    unsafe fn shader_from_src(src: &str, shader_type: GLenum) -> GLuint {
        let id = gl::CreateShader(shader_type);
        // Attempt to compile the shader
        let c_string = CString::new(src).unwrap();
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

        id
    }

    pub unsafe fn bind(&self) {
        gl::UseProgram(self.id);
    }

    pub unsafe fn get_uniform_location(&self, name: *const GLchar) -> GLint {
        gl::GetUniformLocation(self.id, name)
    }
}

impl Drop for GlProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
            gl::DeleteShader(self.vert_shader);
            gl::DeleteShader(self.frag_shader);
        }
    }
}
