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
    gb_thread: Rc<RefCell<ceres_std::GbThread>>,
    renderer: RefCell<Option<Renderer>>,
    shader_changed: RefCell<Option<ShaderMode>>,
    callbacks: RefCell<Option<gtk::TickCallbackId>>,
    buffer: Arc<Mutex<Box<[u8]>>>,
    shader: RefCell<ShaderMode>,
    model: RefCell<ceres_std::Model>,
}

impl GlArea {
    pub fn play(&self) {
        let widget = self.obj();

        *self.callbacks.borrow_mut() = Some(widget.add_tick_callback(move |gl_area, _| {
            gl_area.queue_draw();

            glib::ControlFlow::Continue
        }));

        self.gb_thread.borrow_mut().resume().unwrap();
    }

    pub fn pause(&self) {
        self.gb_thread.borrow_mut().pause().unwrap();

        if let Some(tick_id) = self.callbacks.borrow_mut().take() {
            tick_id.remove();
        }
    }

    pub fn set_model(&self, model: ceres_std::Model) {
        let mut thread = self.gb_thread.borrow_mut();
        thread.change_model(model);
        *self.model.borrow_mut() = model;
    }

    pub fn model(&self) -> ceres_std::Model {
        *self.model.borrow()
    }

    pub fn set_shader(&self, mode: ShaderMode) {
        self.shader_changed.replace(Some(mode));
        self.shader.replace(mode);
    }

    pub fn shader(&self) -> ShaderMode {
        *self.shader.borrow()
    }

    pub const fn gb_thread(&self) -> &Rc<RefCell<ceres_std::GbThread>> {
        &self.gb_thread
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
    type Type = super::GlArea;
    type ParentType = gtk::GLArea;

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
        }
    }
}

impl ObjectImpl for GlArea {}

impl WidgetImpl for GlArea {
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

    fn unrealize(&self) {
        self.pause();
        self.parent_unrealize();
    }

    // TODO: is this right?
    fn request_mode(&self) -> gtk::SizeRequestMode {
        gtk::SizeRequestMode::ConstantSize
    }

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
            .resize_viewport(width as u32, height as u32);
    }
}
