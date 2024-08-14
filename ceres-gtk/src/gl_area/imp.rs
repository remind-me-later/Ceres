use super::renderer::PxScaleMode;
use super::renderer::Renderer;
use crate::audio;
use ceres_core::Gb;
use gtk::glib;
use gtk::glib::Propagation;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::TickCallbackId;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

pub struct GlArea {
    pub gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    pub audio: RefCell<audio::Renderer>,
    pub renderer: RefCell<Option<Renderer>>,
    pub scale_mode: RefCell<PxScaleMode>,
    pub scale_changed: RefCell<bool>,
    pub callbacks: RefCell<Option<TickCallbackId>>,
    pub thread_handle: RefCell<Option<std::thread::JoinHandle<()>>>,
    pub exit: Arc<Mutex<bool>>,
    pub pause_thread: Arc<Mutex<bool>>,
}

fn gb_loop(
    gb: Arc<Mutex<Gb<audio::RingBuffer>>>,
    exit: Arc<Mutex<bool>>,
    pause_thread: Arc<Mutex<bool>>,
) {
    while !*exit.lock().unwrap() {
        let begin = std::time::Instant::now();

        if !*pause_thread.lock().unwrap() {
            if let Ok(mut gb) = gb.lock() {
                gb.run_frame();
            }
        }

        let elapsed = begin.elapsed();

        if elapsed < ceres_core::FRAME_DURATION {
            spin_sleep::sleep(ceres_core::FRAME_DURATION - elapsed);
        }
    }
}

impl GlArea {
    pub fn play(&self) {
        let widget = self.obj();

        *self.callbacks.borrow_mut() = Some(widget.add_tick_callback(move |gl_area, _| {
            gl_area.queue_draw();

            glib::ControlFlow::Continue
        }));

        *self.pause_thread.lock().unwrap() = false;

        self.audio.borrow_mut().resume();
    }

    pub fn pause(&self) {
        self.audio.borrow_mut().pause();

        *self.pause_thread.lock().unwrap() = true;

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
        let cart = ceres_core::Cart::default();
        let audio = RefCell::new(audio::Renderer::new());

        let gb = Arc::new(Mutex::new(ceres_core::Gb::new(
            ceres_core::Model::Cgb,
            audio::Renderer::sample_rate(),
            cart,
            audio.borrow().get_ring_buffer(),
        )));

        let pause_thread = Arc::new(Mutex::new(true));
        let exit = Arc::new(Mutex::new(false));

        let thread_handle = {
            let gb = gb.clone();
            let exit = exit.clone();
            let pause_thread = pause_thread.clone();

            RefCell::new(Some(std::thread::spawn(move || {
                gb_loop(gb, exit, pause_thread)
            })))
        };

        Self {
            gb,
            audio,
            renderer: Default::default(),
            scale_mode: Default::default(),
            scale_changed: Default::default(),
            callbacks: Default::default(),
            thread_handle,
            exit,
            pause_thread,
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
        *self.renderer.borrow_mut() = None;

        *self.exit.lock().unwrap() = true;

        if let Some(tick_id) = self.callbacks.borrow_mut().take() {
            tick_id.remove();
        }

        self.thread_handle.take().unwrap().join().unwrap();

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
    fn render(&self, _context: &gtk::gdk::GLContext) -> Propagation {
        let mut rf = self.renderer.borrow_mut();
        let rend = rf.as_mut().unwrap();

        if *self.scale_changed.borrow() {
            rend.choose_scale_mode(*self.scale_mode.borrow());
            *self.scale_changed.borrow_mut() = false;
        }

        if let Ok(gb) = self.gb.lock() {
            let rgba = gb.pixel_data_rgba();

            rend.draw_frame(rgba);
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
