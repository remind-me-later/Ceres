use {
    gl::types::{GLint, GLuint},
    glutin::{
        dpi::PhysicalSize,
        event_loop::EventLoop,
        window::{Fullscreen, WindowBuilder},
        ContextBuilder, PossiblyCurrent, WindowedContext,
    },
    std::{
        cmp::min,
        ffi::{CStr, CString},
        ptr,
    },
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

pub struct Renderer {
    ctx: WindowedContext<PossiblyCurrent>,
    vao: GLuint,
    program: Shader,
    texture: GLuint,
}

impl Renderer {
    pub fn new(event_loop: &EventLoop<()>) -> Self {
        let window_builder = WindowBuilder::new()
            .with_title(super::CERES_STR)
            .with_inner_size(PhysicalSize {
                width: PX_WIDTH as i32 * 4,
                height: PX_HEIGHT as i32 * 4,
            })
            .with_min_inner_size(PhysicalSize {
                width: PX_WIDTH as i32,
                height: PX_HEIGHT as i32,
            });

        let context_builder = ContextBuilder::new();
        let ctx = context_builder
            .with_vsync(true)
            .build_windowed(window_builder, event_loop)
            .unwrap();

        let ctx = unsafe { ctx.make_current().unwrap() };

        gl::load_with(|s| ctx.get_proc_address(s).cast());

        let program = Shader::new();

        let mut vao = 0;
        let mut texture = 0;

        unsafe {
            // create vao
            gl::GenVertexArrays(1, &mut vao);
            gl::BindVertexArray(vao);

            // create texture
            gl::GenTextures(1, &mut texture);
            gl::BindTexture(gl::TEXTURE_2D, texture);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as i32);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as i32);
        }

        let mut res = Self {
            ctx,
            vao,
            program,
            texture,
        };

        res.resize_viewport(PX_WIDTH * MUL, PX_HEIGHT * MUL);

        res
    }

    pub fn toggle_fullscreen(&mut self) {
        let in_fullscreen = self.ctx.window().fullscreen();

        match in_fullscreen {
            Some(_) => self.ctx.window().set_fullscreen(None),
            None => self
                .ctx
                .window()
                .set_fullscreen(Some(Fullscreen::Borderless(None))),
        }

        let size = self.ctx.window().inner_size();
        self.resize_viewport(size.width, size.height);
    }

    pub fn resize_viewport(&mut self, width: u32, height: u32) {
        let mul = min(width / PX_WIDTH, height / PX_HEIGHT);
        let img_w = PX_WIDTH * mul;
        let img_h = PX_HEIGHT * mul;
        let a = img_w as f32 / width as f32;
        let b = img_h as f32 / height as f32;

        unsafe {
            gl::Viewport(0, 0, width as i32, height as i32);
            self.program.bind();
            gl::Uniform2f(self.program.transform_loc, a, b);
        }

        self.ctx.resize(PhysicalSize { width, height });
    }

    pub fn draw_frame(&mut self, rgba: *const u8) {
        unsafe {
            // TODO: texture streaming
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                rgba.cast(),
            );

            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
            self.program.bind();
            gl::BindVertexArray(self.vao);
            gl::DrawArrays(gl::TRIANGLE_STRIP, 0, 4);
        }

        self.ctx.swap_buffers().unwrap();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}

struct Shader {
    id: GLuint,
    transform_loc: GLint,
    vert_id: GLuint,
    frag_id: GLuint,
}

impl Shader {
    pub fn new() -> Self {
        unsafe {
            // compile fragment shader
            let vert_id = gl::CreateShader(gl::VERTEX_SHADER);
            let vert_src = CString::new(include_str!("shader/vs.vert")).unwrap();
            gl::ShaderSource(vert_id, 1, &(vert_src.as_ptr().cast()), ptr::null());
            gl::CompileShader(vert_id);
            Self::check_compile(vert_id, true);

            // compile fragment shader
            let frag_id = gl::CreateShader(gl::FRAGMENT_SHADER);
            let frag_src = CString::new(include_str!("shader/fs.frag")).unwrap();
            gl::ShaderSource(frag_id, 1, &(frag_src.as_ptr().cast()), ptr::null());
            gl::CompileShader(frag_id);
            Self::check_compile(frag_id, true);

            // link program
            let id = gl::CreateProgram();
            gl::AttachShader(id, vert_id);
            gl::AttachShader(id, frag_id);
            gl::LinkProgram(id);
            Self::check_compile(id, false);

            // get transform
            let transform_loc = gl::GetUniformLocation(id, b"transform\0".as_ptr().cast());

            Self {
                id,
                transform_loc,
                vert_id,
                frag_id,
            }
        }
    }

    unsafe fn check_compile(id: GLuint, is_shader: bool) {
        let mut status = gl::FALSE as i32;

        if is_shader {
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut status);
        } else {
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut status);
        }

        if status == gl::TRUE as i32 {
            return;
        }

        let mut buf = [0; 1024];

        if is_shader {
            gl::GetShaderInfoLog(id, 1024, ptr::null_mut(), buf.as_mut_ptr().cast());
        } else {
            gl::GetProgramInfoLog(id, 1024, ptr::null_mut(), buf.as_mut_ptr().cast());
        }

        let msg = CStr::from_bytes_with_nul(&buf)
            .expect("ShaderInfoLog not valid utf8")
            .to_str()
            .unwrap();

        panic!("{msg}");
    }

    pub unsafe fn bind(&self) {
        gl::UseProgram(self.id);
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
            gl::DeleteShader(self.vert_id);
            gl::DeleteShader(self.frag_id);
        }
    }
}
