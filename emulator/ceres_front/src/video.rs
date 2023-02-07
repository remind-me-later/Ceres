use crate::CERES_STYLIZED;
use sdl2::{
    video::{GLContext, GLProfile, SwapInterval, Window},
    Sdl,
};
use std::{
    cmp::min,
    ffi::{CStr, CString},
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub struct Renderer {
    program: Program,
    _vao: gl::types::GLuint,
    _texture: gl::types::GLuint,
    uniform_loc: gl::types::GLint,
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

        let sdl_ctx = sdl_window.gl_create_context().unwrap();

        debug_assert_eq!(gl_attr.context_profile(), GLProfile::Core);
        debug_assert_eq!(gl_attr.context_version(), (4, 6));

        video_subsystem
            .gl_set_swap_interval(SwapInterval::VSync)
            .unwrap();

        unsafe {
            gl::load_with(|symbol| video_subsystem.gl_get_proc_address(symbol).cast());

            // create vao
            let mut vao = 0;
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            let vs = Shader::from_vert_source(
                CStr::from_bytes_with_nul(
                    concat!(include_str!("../shader/vs.vert"), '\0').as_bytes(),
                )
                .unwrap(),
            )
            .unwrap();
            let fs = Shader::from_frag_source(
                CStr::from_bytes_with_nul(
                    concat!(include_str!("../shader/fs.frag"), '\0').as_bytes(),
                )
                .unwrap(),
            )
            .unwrap();

            let shaders = vec![vs, fs];

            // create program
            let program = Program::from_shaders(&shaders).unwrap();
            program.use_program();

            // create texture
            let mut texture = 0;
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);

            // get uniform location
            let uniform_loc = gl::GetUniformLocation(program.id(), b"transform\0".as_ptr().cast());

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);

            let mut res = Self {
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
            gl::Viewport(0, 0, width as i32, height as i32);

            self.program.use_program();

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

            gl::Uniform2f(self.uniform_loc, x, y);
        }
    }

    pub fn render(&mut self, rgb: &[u8]) {
        unsafe {
            // TODO: texture streaming
            //self.gl::bind_texture(glow::TEXTURE_2D, Some(self.texture));
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGB as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                gl::RGB,
                gl::UNSIGNED_BYTE,
                rgb.as_ptr().cast(),
            );

            gl::Clear(gl::COLOR_BUFFER_BIT);
            //self.gl::use_program(Some(self.program));
            //self.gl::bind_vertex_array(Some(self.vao));
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);

            self.sdl_window.gl_swap_window();
        }
    }
}

pub struct Program {
    id: gl::types::GLuint,
}

impl Program {
    pub fn from_shaders(shaders: &[Shader]) -> Result<Program, String> {
        let program_id = unsafe { gl::CreateProgram() };

        for shader in shaders {
            unsafe {
                gl::AttachShader(program_id, shader.id());
            }
        }

        unsafe {
            gl::LinkProgram(program_id);
        }

        let mut success: gl::types::GLint = 1;
        unsafe {
            gl::GetProgramiv(program_id, gl::LINK_STATUS, &mut success);
        }

        if success == 0 {
            let mut len: gl::types::GLint = 0;
            unsafe {
                gl::GetProgramiv(program_id, gl::INFO_LOG_LENGTH, &mut len);
            }

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetProgramInfoLog(
                    program_id,
                    len,
                    std::ptr::null_mut(),
                    error.as_ptr() as *mut gl::types::GLchar,
                );
            }

            return Err(error.to_string_lossy().into_owned());
        }

        for shader in shaders {
            unsafe {
                gl::DetachShader(program_id, shader.id());
            }
        }

        Ok(Program { id: program_id })
    }

    pub fn id(&self) -> gl::types::GLuint {
        self.id
    }

    pub fn use_program(&self) {
        unsafe {
            gl::UseProgram(self.id);
        }
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}

pub struct Shader {
    id: gl::types::GLuint,
}

impl Shader {
    pub fn from_source(source: &CStr, kind: gl::types::GLenum) -> Result<Shader, String> {
        let id = shader_from_source(source, kind)?;
        Ok(Shader { id })
    }

    pub fn from_vert_source(source: &CStr) -> Result<Shader, String> {
        Shader::from_source(source, gl::VERTEX_SHADER)
    }

    pub fn from_frag_source(source: &CStr) -> Result<Shader, String> {
        Shader::from_source(source, gl::FRAGMENT_SHADER)
    }

    pub fn id(&self) -> gl::types::GLuint {
        self.id
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}

fn shader_from_source(source: &CStr, kind: gl::types::GLenum) -> Result<gl::types::GLuint, String> {
    let id = unsafe { gl::CreateShader(kind) };
    unsafe {
        gl::ShaderSource(id, 1, &source.as_ptr(), std::ptr::null());
        gl::CompileShader(id);
    }

    let mut success: gl::types::GLint = 1;
    unsafe {
        gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
    }

    if success == 0 {
        let mut len: gl::types::GLint = 0;
        unsafe {
            gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len);
        }

        let error = create_whitespace_cstring_with_len(len as usize);

        unsafe {
            gl::GetShaderInfoLog(
                id,
                len,
                std::ptr::null_mut(),
                error.as_ptr() as *mut gl::types::GLchar,
            );
        }

        return Err(error.to_string_lossy().into_owned());
    }

    Ok(id)
}

fn create_whitespace_cstring_with_len(len: usize) -> CString {
    // allocate buffer of correct size
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    // fill it with len spaces
    buffer.extend([b' '].iter().cycle().take(len));
    // convert buffer to CString
    unsafe { CString::from_vec_unchecked(buffer) }
}
