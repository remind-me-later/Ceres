extern crate alloc;

use alloc::string::String;
use core::fmt::{Display, Formatter, Result};

#[derive(Debug)]
pub enum Error {
    ShaderCompile { msg: String },
    ShaderLink { msg: String },
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        use Error::*;
        match self {
            ShaderCompile { msg } => write!(f, "opengl couldn't compile shaders: {msg}"),
            ShaderLink { msg } => write!(f, "opnegl couldn't link shaders: {msg}"),
        }
    }
}
