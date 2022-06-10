use {
    gl::types::{GLint, GLuint},
    sdl2::{
        video::{GLContext, SwapInterval, Window},
        Sdl, VideoSubsystem,
    },
    std::{
        cmp::min,
        ffi::{CStr, CString},
        mem::ManuallyDrop,
        ptr,
    },
};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as _;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as _;
const MUL: u32 = 4;

pub struct Renderer {
    win: Window,
    vao: GLuint,
    program: Shader,
    texture: GLuint,
    video: ManuallyDrop<VideoSubsystem>,
    ctx: ManuallyDrop<GLContext>,
}

impl Renderer {
    pub fn new(sdl: &Sdl) -> Self {
        let video = ManuallyDrop::new(sdl.video().unwrap());

        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(4, 6);
        gl_attr.set_depth_size(0);
        gl_attr.set_context_flags().forward_compatible().set();

        if !cfg!(debug_assertions) {
            gl_attr.set_context_no_error(true);
        }

        let mut win = video
            .window(crate::CERES_STR, PX_WIDTH * MUL, PX_HEIGHT * MUL)
            .opengl()
            .position_centered()
            .resizable()
            .build()
            .unwrap();
        win.set_minimum_size(PX_WIDTH, PX_HEIGHT).unwrap();

        let ctx = ManuallyDrop::new(win.gl_create_context().unwrap());
        win.gl_make_current(&ctx).unwrap();
        video.gl_set_swap_interval(SwapInterval::VSync).unwrap();

        gl::load_with(|s| video.gl_get_proc_address(s).cast());

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
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as _);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as _);
        }

        let mut res = Self {
            win,
            vao,
            program,
            texture,
            video,
            ctx,
        };

        res.resize_viewport(PX_WIDTH * MUL, PX_HEIGHT * MUL);

        res
    }

    pub fn toggle_fullscreen(&mut self, on: bool) {
        if on {
            self.win
                .set_fullscreen(sdl2::video::FullscreenType::Desktop)
                .unwrap();
        } else {
            self.win
                .set_fullscreen(sdl2::video::FullscreenType::Off)
                .unwrap();
        }

        let (w, h) = self.win.size();
        self.resize_viewport(w, h);
    }

    pub fn resize_viewport(&mut self, w: u32, h: u32) {
        let mul = min(w / PX_WIDTH, h / PX_HEIGHT);
        let img_w = PX_WIDTH * mul;
        let img_h = PX_HEIGHT * mul;
        let a = img_w as f32 / w as f32;
        let b = img_h as f32 / h as f32;

        unsafe {
            gl::Viewport(0, 0, w as i32, h as i32);
            self.program.bind();
            gl::Uniform2f(self.program.transform_loc, a, b);
        }
    }

    pub fn draw_frame(&mut self, rgba: *const u8) {
        unsafe {
            // TODO: texture streaming
            gl::BindTexture(gl::TEXTURE_2D, self.texture);
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as _,
                PX_WIDTH as _,
                PX_HEIGHT as _,
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

        self.win.gl_swap_window();
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.vao);
            ManuallyDrop::drop(&mut self.ctx);
            ManuallyDrop::drop(&mut self.video);
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
            let src = CString::new(include_str!("shader/vs.vert")).unwrap();
            gl::ShaderSource(vert_id, 1, &(src.as_ptr().cast()), ptr::null());
            gl::CompileShader(vert_id);
            Self::check_compile(vert_id, true);

            // compile fragment shader
            let frag_id = gl::CreateShader(gl::FRAGMENT_SHADER);
            let src = CString::new(include_str!("shader/fs.frag")).unwrap();
            gl::ShaderSource(frag_id, 1, &(src.as_ptr().cast()), ptr::null());
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
        let mut status = gl::FALSE as _;

        if is_shader {
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut status);
        } else {
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut status);
        }

        if status == gl::TRUE as _ {
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
