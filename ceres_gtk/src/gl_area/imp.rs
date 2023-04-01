use super::renderer::Renderer;
use super::renderer::PxScaleMode;
use crate::audio;
use ceres_core::Gb;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::sync::Arc;

pub struct GlArea {
    pub gb: Arc<Mutex<Gb>>,
    pub audio: Arc<Mutex<audio::Renderer>>,
    pub renderer: RefCell<Option<Renderer>>,
    pub scale_mode: RefCell<PxScaleMode>,
    pub scale_changed: RefCell<bool>,
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
        let cart = ceres_core::Cart::default();
        let gb = Arc::new(Mutex::new(ceres_core::Gb::new(
            ceres_core::Model::Cgb,
            audio::Renderer::sample_rate(),
            cart,
        )));
        let audio = Arc::new(Mutex::new(audio::Renderer::new(Arc::clone(&gb)).unwrap()));

        Self {
            gb,
            audio,
            renderer: Default::default(),
            scale_mode: Default::default(),
            scale_changed: Default::default(),
        }
    }
}

impl ObjectImpl for GlArea {
    fn constructed(&self) {
        self.parent_constructed();

        let widget = self.obj();

        widget.add_tick_callback(move |p, _| {
            p.queue_draw();
            glib::Continue(true)
        });
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
        *self.renderer.borrow_mut() = None;

        self.parent_unrealize();
    }

    // TODO: is this right?
    fn request_mode(&self) -> gtk::SizeRequestMode {
        gtk::SizeRequestMode::ConstantSize
    }

    fn measure(&self, orientation: gtk::Orientation, _for_size: i32) -> (i32, i32, i32, i32) {
        const MULTIPLIER: i32 = 3;

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
    fn render(&self, _context: &gtk::gdk::GLContext) -> bool {
        let mut rf = self.renderer.borrow_mut();
        let rend = rf.as_mut().unwrap();

        if *self.scale_changed.borrow() {
            rend.choose_scale_mode(*self.scale_mode.borrow());
            *self.scale_changed.borrow_mut() = false;
        }

        let gb = self.gb.lock();
        let rgba = gb.pixel_data_rgba();

        rend.draw_frame(rgba);

        true
    }

    fn resize(&self, width: i32, height: i32) {
        self.renderer
            .borrow_mut()
            .as_mut()
            .unwrap()
            .resize_viewport(width as u32, height as u32);
    }
}
