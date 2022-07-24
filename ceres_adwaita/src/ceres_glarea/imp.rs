use {
    ceres_core::{Gb, Sample},
    gtk::{gdk, gdk_pixbuf, glib, graphene, prelude::*, subclass::prelude::*},
    libadwaita::{glib::Bytes, gtk},
    std::{cell::RefCell, fs::File, io::Read, path::Path, rc::Rc},
};

pub struct CeresAreaData {
    pub gb: &'static mut Gb,
}

impl CeresAreaData {
    pub fn new(path: &std::path::Path, _context: super::CeresArea) -> Self {
        let sav_path = path.with_extension("sav");

        {
            // initialize cartridge
            fn read_file_into(path: &Path, buf: &mut [u8]) -> Result<(), std::io::Error> {
                let mut f = File::open(path)?;
                let _ = f.read(buf).unwrap();
                Ok(())
            }

            read_file_into(path, Gb::cartridge_rom_mut()).unwrap();
            read_file_into(&sav_path, Gb::cartridge_ram_mut()).ok();
        }

        let gb = Gb::new(ceres_core::Model::Cgb, apu_frame_callback, 48000).unwrap();

        Self { gb }
    }
}

#[derive(Default)]
pub struct CeresArea {
    pub data: Rc<RefCell<Option<CeresAreaData>>>,
}

#[glib::object_subclass]
impl ObjectSubclass for CeresArea {
    const NAME: &'static str = "CeresArea";
    type Type = super::CeresArea;
    type Interfaces = (gdk::Paintable,);
    type ParentType = gtk::Widget;
}

impl CeresArea {
    pub fn set_rom_path(&self, path: &std::path::Path) {
        let context = self.instance();
        let data = CeresAreaData::new(path, context);
        *self.data.borrow_mut() = Some(data);
    }

    pub fn get_frame(&self) {
        if let Some(data) = self.data.borrow_mut().as_mut() {
            data.gb.run_frame();
        }
    }
}

impl WidgetImpl for CeresArea {}

impl ObjectImpl for CeresArea {}

impl PaintableImpl for CeresArea {
    fn flags(&self, _paintable: &Self::Type) -> gdk::PaintableFlags {
        gdk::PaintableFlags::SIZE
    }

    fn intrinsic_width(&self, _paintable: &Self::Type) -> i32 {
        ceres_core::PX_WIDTH as i32
    }

    fn intrinsic_height(&self, _paintable: &Self::Type) -> i32 {
        ceres_core::PX_HEIGHT as i32
    }

    fn snapshot(&self, _paintable: &Self::Type, snapshot: &gdk::Snapshot, width: f64, height: f64) {
        if let Some(data) = self.data.borrow_mut().as_mut() {
            data.gb.run_frame();

            let snapshot = snapshot.downcast_ref::<gtk::Snapshot>().unwrap();

            let bytes = &Bytes::from(data.gb.pixel_data());
            let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(
                bytes,
                gdk_pixbuf::Colorspace::Rgb,
                true,
                8,
                ceres_core::PX_WIDTH as i32,
                ceres_core::PX_HEIGHT as i32,
                ceres_core::PX_WIDTH as i32 * 4,
            );
            let pixbuf = pixbuf
                .scale_simple(width as i32, height as i32, gdk_pixbuf::InterpType::Nearest)
                .unwrap();

            let texture = gdk::Texture::for_pixbuf(&pixbuf);

            snapshot.append_texture(
                &texture,
                &graphene::Rect::new(0_f32, 0_f32, width as f32, height as f32),
            );
        }
    }
}

#[inline]
fn apu_frame_callback(_l: Sample, _r: Sample) {
    // let audio = unsafe { &mut *AUDIO };
    // audio.push_frame(l, r);
}
