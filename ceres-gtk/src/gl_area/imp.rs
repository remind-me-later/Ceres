use super::renderer::PxScaleMode;
use super::renderer::Renderer;
use gtk::glib;
use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::TickCallbackId;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub struct PainterCallbackImpl {
    buffer: Arc<Mutex<Box<[u8]>>>,
}

impl PainterCallbackImpl {
    pub fn new(buffer: Arc<Mutex<Box<[u8]>>>) -> Self {
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
    pub gb_thread: Rc<RefCell<ceres_std::GbThread>>,
    pub renderer: RefCell<Option<Renderer>>,
    pub scale_mode: RefCell<PxScaleMode>,
    pub scale_changed: RefCell<bool>,
    pub callbacks: RefCell<Option<TickCallbackId>>,
    pub buffer: Arc<Mutex<Box<[u8]>>>,
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
            vec![0u8; ceres_std::PIXEL_BUFFER_SIZE].into_boxed_slice(),
        ));

        let gb_thread = Rc::new(RefCell::new(
            ceres_std::GbThread::new(
                ceres_core::Model::Cgb,
                None,
                None,
                PainterCallbackImpl::new(buffer.clone()),
            )
            .expect("Failed to create GbThread"),
        ));

        Self {
            gb_thread,
            buffer,
            renderer: Default::default(),
            scale_mode: Default::default(),
            scale_changed: Default::default(),
            callbacks: Default::default(),
        }
    }
}

impl ObjectImpl for GlArea {
    fn constructed(&self) {
        self.parent_constructed();
        self.play();
    }
}

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
    }

    fn unrealize(&self) {
        if let Some(tick_id) = self.callbacks.borrow_mut().take() {
            tick_id.remove();
        }

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
                let minimum_size = ceres_core::PX_WIDTH as i32;
                let natural_size = minimum_size * MULTIPLIER;

                (minimum_size, natural_size, -1, -1)
            }
            gtk::Orientation::Vertical => {
                let minimum_size = ceres_core::PX_HEIGHT as i32;
                let natural_size = minimum_size * MULTIPLIER;

                (minimum_size, natural_size, -1, -1)
            }
            _ => unreachable!(),
        }
    }
}

impl GLAreaImpl for GlArea {
    fn render(&self, _context: &gtk::gdk::GLContext) -> Propagation {
        let mut rf = self.renderer.borrow_mut();
        let rend = rf.as_mut().unwrap();

        if *self.scale_changed.borrow() {
            rend.choose_scale_mode(*self.scale_mode.borrow());
            *self.scale_changed.borrow_mut() = false;
        }

        if let Ok(rgba) = self.buffer.lock() {
            rend.draw_frame(&rgba);
        }

        Propagation::Proceed
    }

    fn resize(&self, width: i32, height: i32) {
        self.renderer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .resize_viewport(width as u32, height as u32);
    }
}
