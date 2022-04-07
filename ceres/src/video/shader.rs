extern crate alloc;

use super::Error;
use alloc::borrow::ToOwned;
use alloc::vec::Vec;
use core::ptr;
use gl::types::*;

pub struct Shader<const SHADER_TYPE: GLenum> {
    id: GLuint,
}

impl<const SHADER_TYPE: GLenum> Shader<SHADER_TYPE> {
    pub unsafe fn new(source: &'static [u8]) -> Result<Self, Error> {
        let id = gl::CreateShader(SHADER_TYPE);
        // Attempt to compile the shader
        let mut c_string = source.to_vec();
        c_string.push(b'\0');
        gl::ShaderSource(id, 1, &(c_string.as_ptr() as *const i8), ptr::null());
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

            Err(Error::ShaderCompile {
                msg: core::str::from_utf8(&buf)
                    .unwrap_or("ShaderInfoLog not valid utf8")
                    .to_owned(),
            })
        } else {
            Ok(Self { id })
        }
    }

    pub fn id(&self) -> GLuint {
        self.id
    }
}

impl<const SHADER_TYPE: GLenum> Drop for Shader<SHADER_TYPE> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}
