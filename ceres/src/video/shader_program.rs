use super::shader::Shader;
use super::Error;
use core::ptr;
use gl::types::*;

pub struct ShaderProgram {
    id: GLuint,
    // keep them alive
    _vertex_shader: Shader<{ gl::VERTEX_SHADER }>,
    _fragment_shader: Shader<{ gl::FRAGMENT_SHADER }>,
}

impl ShaderProgram {
    pub unsafe fn new(
        vertex_shader: Shader<{ gl::VERTEX_SHADER }>,
        fragment_shader: Shader<{ gl::FRAGMENT_SHADER }>,
    ) -> Result<Self, Error> {
        let id = gl::CreateProgram();
        gl::AttachShader(id, vertex_shader.id());
        gl::AttachShader(id, fragment_shader.id());
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
            Err(Error::ShaderLink {
                msg: core::str::from_utf8(&buf)
                    .unwrap_or("ShaderInfoLog not valid utf8")
                    .to_owned(),
            })
        } else {
            Ok(Self {
                id,
                _vertex_shader: vertex_shader,
                _fragment_shader: fragment_shader,
            })
        }
    }

    pub unsafe fn use_program(&self) {
        gl::UseProgram(self.id);
    }

    pub unsafe fn get_uniform_location(&self, name: *const GLchar) -> GLint {
        gl::GetUniformLocation(self.id, name)
    }
}

impl Drop for ShaderProgram {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}
