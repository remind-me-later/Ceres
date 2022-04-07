mod error;
mod shader;
mod shader_program;

use core::{ffi::c_void, mem, ptr};
use error::Error;
use gl::types::*;
use shader::Shader;
use shader_program::ShaderProgram;

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

pub trait Context {
    fn get_proc_address(&mut self, procname: &str) -> *const c_void;
    fn swap_buffers(&mut self);
    fn make_current(&mut self);
    fn resize(&mut self, width: u32, height: u32);
}

pub struct Renderer<C: Context> {
    context: C,
    vbo: GLuint,
    vao: GLuint,
    ebo: GLuint,
    shader_program: ShaderProgram,
    texture: GLuint,
    transform_location: GLint,
}

impl<C: Context> Renderer<C> {
    pub fn new(
        mut context: C,
        initial_window_width: u32,
        initial_window_height: u32,
    ) -> Result<Renderer<C>, Error> {
        context.make_current();

        gl::load_with(|procname| context.get_proc_address(procname));

        let mut vbo = 0;
        let mut vao = 0;
        let mut ebo = 0;
        let mut texture = 0;
        let transform_name = "transform\0";
        let transform_location;
        let program;

        unsafe {
            let vertex_shader = Shader::new(include_bytes!("video/shader/vs.vert"))?;
            let fragment_shader = Shader::new(include_bytes!("video/shader/fs.frag"))?;
            program = ShaderProgram::new(vertex_shader, fragment_shader)?;

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
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);

            // scaling behaviour
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);

            // create transform
            transform_location = program.get_uniform_location(transform_name.as_ptr().cast());
        }

        let mut video_renderer = Renderer {
            vbo,
            vao,
            ebo,
            shader_program: program,
            context,
            texture,
            transform_location,
        };

        video_renderer.resize_viewport(initial_window_width, initial_window_height);

        Ok(video_renderer)
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let gb_width = ceres_core::SCREEN_WIDTH as u32;
        let gb_height = ceres_core::SCREEN_HEIGHT as u32;
        let multiplier = core::cmp::min(width / gb_width, height / gb_height);
        let surface_width = gb_width * multiplier;
        let surface_height = gb_height * multiplier;

        let x = surface_width as f32 / width as f32;
        let y = surface_height as f32 / height as f32;

        // hand-written scale matrix
        let transform: [f32; 16] = [
            x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ];

        unsafe {
            self.shader_program.use_program();
            gl::Viewport(0, 0, width as i32, height as i32);
            gl::UniformMatrix4fv(self.transform_location, 1, gl::FALSE, transform.as_ptr());
        }

        self.context.resize(width, height);
    }

    pub fn update_texture(&mut self, rgba_pixel_data: &[u8]) {
        // TODO: texture streaming
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as GLint,
                ceres_core::SCREEN_WIDTH as GLint,
                ceres_core::SCREEN_HEIGHT as GLint,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba_pixel_data.as_ptr().cast(),
            );
        }
    }

    #[allow(clippy::cast_possible_wrap)]
    pub fn draw(&mut self) {
        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            self.shader_program.use_program();
            gl::DrawElements(gl::TRIANGLES, 6, gl::UNSIGNED_BYTE, ptr::null());
        }

        self.context.swap_buffers();
    }
}

impl<C: Context> Drop for Renderer<C> {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteBuffers(1, &self.ebo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}
