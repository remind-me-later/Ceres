use super::renderer::{Renderer, ShaderMode};
use gtk::{glib, prelude::*, subclass::prelude::*};
use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

pub struct PainterCallbackImpl {
    buffer: Arc<Mutex<Box<[u8]>>>,
}

impl PainterCallbackImpl {
    pub const fn new(buffer: Arc<Mutex<Box<[u8]>>>) -> Self {
        Self { buffer }
    }
}

impl ceres_std::PainterCallback for PainterCallbackImpl {
    fn paint(&self, pixel_data_rgba: &[u8]) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.copy_from_slice(pixel_data_rgba);
        }
    }

    fn request_repaint(&self) {}
}

pub struct GlArea {
    buffer: Arc<Mutex<Box<[u8]>>>,
    callbacks: RefCell<Option<gtk::TickCallbackId>>,
    gb_thread: Rc<RefCell<ceres_std::GbThread>>,
    is_running: RefCell<bool>,
    model: RefCell<ceres_std::Model>,
    pixel_perfect: RefCell<bool>,
    renderer: RefCell<Option<Renderer>>,
    shader: RefCell<ShaderMode>,
    shader_changed: RefCell<Option<ShaderMode>>,
}

impl GlArea {
    pub const fn gb_thread(&self) -> &Rc<RefCell<ceres_std::GbThread>> {
        &self.gb_thread
    }

    fn pause(&self) {
        self.gb_thread.borrow_mut().pause().unwrap();

        if let Some(tick_id) = self.callbacks.borrow_mut().take() {
            tick_id.remove();
        }

        *self.is_running.borrow_mut() = false;
    }

    fn play(&self) {
        let widget = self.obj();

        *self.callbacks.borrow_mut() = Some(widget.add_tick_callback(move |gl_area, _| {
            gl_area.queue_draw();

            glib::ControlFlow::Continue
        }));

        self.gb_thread.borrow_mut().resume().unwrap();

        *self.is_running.borrow_mut() = true;
    }
}

impl Default for GlArea {
    fn default() -> Self {
        Self::new()
    }
}

#[glib::object_subclass]
impl ObjectSubclass for GlArea {
    const NAME: &'static str = "CeresGlArea";
    type ParentType = gtk::GLArea;
    type Type = super::GlArea;

    fn new() -> Self {
        let buffer = Arc::new(Mutex::new(
            vec![0_u8; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let gb_thread = Rc::new(RefCell::new(
            ceres_std::GbThread::new(
                ceres_std::Model::Cgb,
                None,
                None,
                PainterCallbackImpl::new(Arc::clone(&buffer)),
            )
            .expect("Failed to create GbThread"),
        ));

        Self {
            gb_thread,
            buffer,
            renderer: Default::default(),
            shader_changed: Default::default(),
            callbacks: Default::default(),
            shader: RefCell::new(ShaderMode::default()),
            model: RefCell::new(ceres_std::Model::default()),
            is_running: RefCell::new(false),
            pixel_perfect: RefCell::new(false),
        }
    }
}

impl ObjectImpl for GlArea {
    fn properties() -> &'static [glib::ParamSpec] {
        use std::sync::OnceLock;
        static PROPERTIES: OnceLock<Vec<glib::ParamSpec>> = OnceLock::new();

        PROPERTIES.get_or_init(|| {
            vec![
                glib::ParamSpecString::builder("shader-mode")
                    .nick("Shader Mode")
                    .blurb("The shader mode to use for rendering")
                    .default_value(Some("Nearest"))
                    .build(),
                glib::ParamSpecString::builder("gb-model")
                    .nick("GameBoy Model")
                    .blurb("The GameBoy model to emulate")
                    .default_value(Some("cgb"))
                    .build(),
                glib::ParamSpecBoolean::builder("emulator-running")
                    .nick("Emulator Running")
                    .blurb("Whether the emulator is currently running")
                    .default_value(false)
                    .build(),
                glib::ParamSpecBoolean::builder("pixel-perfect")
                    .nick("Pixel Perfect")
                    .blurb("Whether to use pixel-perfect scaling")
                    .default_value(false)
                    .build(),
            ]
        })
    }

    fn property(&self, _id: usize, pspec: &glib::ParamSpec) -> glib::Value {
        match pspec.name() {
            "shader-mode" => self.shader.borrow().to_string().to_value(),
            "gb-model" => match *self.model.borrow() {
                ceres_std::Model::Dmg => "dmg",
                ceres_std::Model::Mgb => "mgb",
                ceres_std::Model::Cgb => "cgb",
            }
            .to_value(),
            "emulator-running" => self.is_running.borrow().to_value(),
            "pixel-perfect" => self.pixel_perfect.borrow().to_value(),
            _ => {
                eprintln!("Unknown property: {}", pspec.name());
                glib::Value::from("")
            }
        }
    }

    fn set_property(&self, _id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
        match pspec.name() {
            "shader-mode" => {
                let shader_str = value.get::<String>().unwrap();
                let mode = ShaderMode::from(shader_str.as_str());
                self.shader_changed.replace(Some(mode));
                self.shader.replace(mode);
            }
            "gb-model" => {
                let model_str = value.get::<String>().unwrap();
                let model = match model_str.as_str() {
                    "dmg" => ceres_std::Model::Dmg,
                    "mgb" => ceres_std::Model::Mgb,
                    "cgb" => ceres_std::Model::Cgb,
                    _ => ceres_std::Model::Cgb,
                };
                let mut thread = self.gb_thread.borrow_mut();
                thread.change_model(model);
                *self.model.borrow_mut() = model;
            }
            "emulator-running" => {
                let is_running = value.get::<bool>().unwrap();
                if is_running {
                    self.play();
                } else {
                    self.pause();
                }
            }
            "pixel-perfect" => {
                let use_pixel_perfect = value.get::<bool>().unwrap();
                self.pixel_perfect.replace(use_pixel_perfect);
                if let Some(renderer) = self.renderer.borrow_mut().as_mut() {
                    let size = renderer.current_size();
                    renderer.resize_viewport(size.0, size.1, use_pixel_perfect);
                }
            }
            _ => {
                eprintln!("Unknown property: {}", pspec.name());
            }
        }
    }
}

impl WidgetImpl for GlArea {
    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        const MULTIPLIER: i32 = 1;

        match orientation {
            gtk::Orientation::Horizontal => {
                const MINIMUM_SIZE: i32 = ceres_std::PX_WIDTH as i32;
                const NATURAL_SIZE: i32 = MINIMUM_SIZE * MULTIPLIER;

                (MINIMUM_SIZE, NATURAL_SIZE, -1, -1)
            }
            gtk::Orientation::Vertical => {
                const MINIMUM_SIZE: i32 = ceres_std::PX_HEIGHT as i32;
                const NATURAL_SIZE: i32 = MINIMUM_SIZE * MULTIPLIER;

                (MINIMUM_SIZE, NATURAL_SIZE, -1, -1)
            }
            _ => unreachable!(),
        }
    }

    fn realize(&self) {
        self.parent_realize();

        let widget = self.obj();
        if widget.error().is_some() {
            return;
        }

        widget.set_vexpand(true);
        widget.set_hexpand(true);

        // SAFETY: we know the GdkGLContext exists as we checked for errors above, and we haven't
        // done any operations on it which could lead to glium's state mismatch. (In theory, GTK
        // doesn't do any state-breaking operations on the context either.)
        //
        // We will also ensure glium's context does not outlive the GdkGLContext by destroying it in
        // `unrealize()`.
        widget.make_current();

        *self.renderer.borrow_mut() = Some(Renderer::new());

        self.play();
    }

    // TODO: is this right?
    fn request_mode(&self) -> gtk::SizeRequestMode {
        gtk::SizeRequestMode::ConstantSize
    }

    fn unrealize(&self) {
        self.pause();
        self.parent_unrealize();
    }
}

impl GLAreaImpl for GlArea {
    fn render(&self, _context: &gtk::gdk::GLContext) -> glib::Propagation {
        let mut rf = self.renderer.borrow_mut();
        let rend = rf.as_mut().unwrap();

        if let Some(scale_mode) = self.shader_changed.take() {
            rend.choose_scale_mode(scale_mode);
        }

        if let Ok(rgba) = self.buffer.lock() {
            rend.draw_frame(&rgba);
        }

        glib::Propagation::Proceed
    }

    fn resize(&self, width: i32, height: i32) {
        self.renderer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .resize_viewport(width as u32, height as u32, *self.pixel_perfect.borrow());
    }
}
