use glutin::{
    config::{Config, ConfigTemplateBuilder},
    context::{ContextAttributesBuilder, NotCurrentContext, PossiblyCurrentContext},
    display::{Display, GetGlDisplay},
    prelude::{GlDisplay, NotCurrentGlContextSurfaceAccessor},
    surface::{GlSurface, Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use raw_window_handle::HasRawWindowHandle;
use std::{ffi::CString, num::NonZeroU32};
use winit::{
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use {glow::HasContext, std::cmp::min};

const PX_WIDTH: u32 = ceres_core::PX_WIDTH as u32;
const PX_HEIGHT: u32 = ceres_core::PX_HEIGHT as u32;
const MUL: u32 = 4;

struct Renderer {
    gl: glow::Context,
    program: glow::NativeProgram,
    _vao: glow::NativeVertexArray,
    _texture: glow::NativeTexture,
    uniform_loc: glow::UniformLocation,
    pixel_perfect: bool,
}

impl Renderer {
    fn new<D: GlDisplay>(gl_display: &D) -> Self {
        unsafe {
            let gl = glow::Context::from_loader_function(|symbol| {
                let symbol = CString::new(symbol).unwrap();
                gl_display.get_proc_address(symbol.as_c_str()).cast()
            });

            // create vao
            let vao = gl
                .create_vertex_array()
                .expect("Cannot create vertex array");
            gl.bind_vertex_array(Some(vao));

            // create program
            let program = gl.create_program().expect("Cannot create program");

            let shader_sources = [
                (glow::VERTEX_SHADER, include_str!("../shader/vs.vert")),
                (glow::FRAGMENT_SHADER, include_str!("../shader/fs.frag")),
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

            // create texture
            let texture = gl.create_texture().expect("cannot create texture");
            gl.bind_texture(glow::TEXTURE_2D, Some(texture));
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

            let uniform_loc = gl
                .get_uniform_location(program, "transform")
                .expect("couldn't get location of uniform");

            gl.clear_color(0.0, 0.0, 0.0, 1.0);

            let res = Self {
                gl,
                program,
                _vao: vao,
                _texture: texture,
                uniform_loc,
                pixel_perfect: false,
            };

            res.resize(PX_WIDTH * MUL, PX_HEIGHT * MUL);

            res
        }
    }

    fn resize(&self, width: u32, height: u32) {
        unsafe {
            self.gl.viewport(0, 0, width as i32, height as i32);

            self.gl.use_program(Some(self.program));

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

            self.gl.uniform_2_f32(Some(&self.uniform_loc), x, y);
        }
    }

    fn render(&self, rgb: &[u8]) {
        unsafe {
            // TODO: texture streaming
            //self.gl.bind_texture(glow::TEXTURE_2D, Some(self.texture));
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGB as i32,
                PX_WIDTH as i32,
                PX_HEIGHT as i32,
                0,
                glow::RGB,
                glow::UNSIGNED_BYTE,
                Some(rgb),
            );

            self.gl.clear(glow::COLOR_BUFFER_BIT);
            //self.gl.use_program(Some(self.program));
            //self.gl.bind_vertex_array(Some(self.vao));
            self.gl.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);
        }
    }
}

struct CurrentGl {
    context: PossiblyCurrentContext,
    renderer: Renderer,
    surface: Surface<WindowSurface>,
}

impl CurrentGl {
    fn new(
        window: &Window,
        config: &Config,
        not_current_gl_context: NotCurrentContext,
        display: &Display,
    ) -> Self {
        let (width, height): (u32, u32) = window.inner_size().into();
        let raw_window_handle = window.raw_window_handle();
        let attrs = SurfaceAttributesBuilder::<WindowSurface>::new().build(
            raw_window_handle,
            NonZeroU32::new(width).unwrap(),
            NonZeroU32::new(height).unwrap(),
        );

        let surface = unsafe {
            config
                .display()
                .create_window_surface(config, &attrs)
                .unwrap()
        };

        let gl_context = not_current_gl_context.make_current(&surface).unwrap();

        // Try setting vsync.
        if let Err(res) =
            surface.set_swap_interval(&gl_context, SwapInterval::Wait(NonZeroU32::new(1).unwrap()))
        {
            eprintln!("Error setting vsync: {res:?}");
        }

        // The context needs to be current for the Renderer to set up shaders and
        // buffers. It also performs function loading, which needs a current context on
        // WGL.
        let renderer = Renderer::new(display);

        Self {
            renderer,
            context: gl_context,
            surface,
        }
    }
}

pub struct Gl {
    gl_config: Config,
    gl_display: Display,
    not_current_gl_context: Option<NotCurrentContext>,
    current: Option<CurrentGl>,
    resize_requested: Option<(u32, u32)>,
    // XXX the surface must be dropped before the window.
    maybe_window: Window,
}

impl Gl {
    pub fn new(event_loop: &EventLoop<()>, window_builder: WindowBuilder) -> Self {
        let display_builder =
            glutin_winit::DisplayBuilder::new().with_window_builder(Some(window_builder));

        let template = ConfigTemplateBuilder::new();

        let (maybe_window, gl_config) = display_builder
            .build(event_loop, template, |mut confs| confs.next().unwrap())
            .unwrap();

        let raw_window_handle = maybe_window
            .as_ref()
            .map(HasRawWindowHandle::raw_window_handle);
        let gl_display = gl_config.display();
        let context_attributes = ContextAttributesBuilder::new().build(raw_window_handle);
        let not_current_gl_context = Some(unsafe {
            gl_display
                .create_context(&gl_config, &context_attributes)
                .expect("failed to create context")
        });

        Self {
            gl_config,
            gl_display,
            maybe_window: maybe_window.unwrap(),
            not_current_gl_context,
            current: None,
            resize_requested: None,
        }
    }

    pub fn make_current(&mut self) {
        let current = CurrentGl::new(
            &self.maybe_window,
            &self.gl_config,
            self.not_current_gl_context.take().unwrap(),
            &self.gl_display,
        );

        assert!(self.current.replace(current).is_none());
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        if width != 0 && height != 0 {
            self.resize_requested = Some((width, height));
        }
    }

    pub fn render<F>(&mut self, mut f: F)
    where
        F: FnMut() -> *const [u8],
    {
        if let Some(c) = &self.current {
            if let Some((width, height)) = self.resize_requested {
                c.surface.resize(
                    &c.context,
                    NonZeroU32::new(width).unwrap(),
                    NonZeroU32::new(height).unwrap(),
                );
                c.renderer.resize(width, height);
                self.resize_requested = None;
            }

            let rgb = unsafe { &*f() };
            c.renderer.render(rgb);
            c.surface.swap_buffers(&c.context).unwrap();
        }
    }
}
