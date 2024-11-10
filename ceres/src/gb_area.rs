use crate::{scene, Scaling};
use ceres_core::{Cart, Gb};
use std::{
    io::Read,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering::Relaxed},
        Arc, Mutex,
    },
};
use thread_priority::ThreadBuilderExt;

pub struct GbArea {
    scene: scene::Scene,
    rom_ident: String,
    exiting: Arc<AtomicBool>,
    audio_stream: ceres_audio::Stream,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl GbArea {
    pub fn new(
        model: ceres_core::Model,
        rom_path: Option<&Path>,
        audio_state: &ceres_audio::State,
    ) -> anyhow::Result<Self> {
        let (cart, rom_ident) = if let Some(rom_path) = rom_path {
            let mut cart = Self::cart_from_path(rom_path)?;
            let ident = Self::ident_from_cart(&cart)?;
            if let Ok(ram) = Self::ram_from_dirs_ident(&ident) {
                cart.set_ram(ram)?;
            } else {
                println!("No RAM found for cart {}", ident);
            }

            (cart, ident)
        } else {
            (Cart::default(), String::new())
        };

        let sample_rate = ceres_audio::Stream::sample_rate();
        let mut audio_stream = ceres_audio::Stream::new(audio_state).unwrap();
        let ring_buffer = audio_stream.get_ring_buffer();

        let gb = Arc::new(Mutex::new(Gb::new(model, sample_rate, cart, ring_buffer)));
        audio_stream.resume().unwrap();

        let pause_thread = Arc::new(AtomicBool::new(false));

        let exiting = Arc::new(AtomicBool::new(false));

        let thread_builder = std::thread::Builder::new().name("gb_loop".to_owned());
        let thread_handle = {
            let gb = Arc::clone(&gb);
            let exit = Arc::clone(&exiting);
            let pause_thread = Arc::clone(&pause_thread);

            // std::thread::spawn(move || gb_loop(gb, exit, pause_thread))
            thread_builder
                .spawn_with_priority(thread_priority::ThreadPriority::Max, move |_| {
                    Self::gb_loop(gb, exit, pause_thread);
                })
                .expect("failed to spawn thread")
        };

        let scene = scene::Scene::new(gb, Scaling::default());

        Ok(Self {
            scene,
            rom_ident,
            exiting,
            thread_handle: Some(thread_handle),
            audio_stream,
        })
    }

    // pub fn is_paused(&self) -> bool {
    //     self.pause_thread.load(Relaxed)
    // }

    // pub fn pause(&mut self) {
    //     self.audio_stream.pause();
    //     self.pause_thread.store(true, Relaxed);
    // }

    // pub fn resume(&mut self) {
    //     self.pause_thread.store(false, Relaxed);
    //     self.audio_stream.resume();
    // }

    // pub fn rom_ident(&self) -> &str {
    //     &self.rom_ident
    // }

    pub fn scaling(&self) -> Scaling {
        self.scene.scaling()
    }

    pub fn set_scaling(&mut self, scaling: Scaling) {
        self.scene.set_scaling(scaling);
    }

    pub fn scene(&self) -> &scene::Scene {
        &self.scene
    }

    pub fn change_rom(&mut self, rom_path: &Path, model: ceres_core::Model) -> anyhow::Result<()> {
        let mut cart = Self::cart_from_path(rom_path)?;
        let ident = Self::ident_from_cart(&cart)?;

        if let Ok(ram) = Self::ram_from_dirs_ident(&ident) {
            cart.set_ram(ram).unwrap();
        }

        let sample_rate = ceres_audio::Stream::sample_rate();
        let ring_buffer = self.audio_stream.get_ring_buffer();

        let new_gb = Gb::new(model, sample_rate, cart, ring_buffer);
        self.scene.replace_gb(new_gb);

        Ok(())
    }
}

impl GbArea {
    // In theory can't ever fail because ROM title is always ASCII, in practice I don't know if we check for that on Cart creation
    fn ident_from_cart(cart: &ceres_core::Cart) -> anyhow::Result<String> {
        let mut ident = String::new();
        cart.ascii_title().read_to_string(&mut ident)?;
        ident.push('-');
        ident.push_str(cart.version().to_string().as_str());
        ident.push('-');
        ident.push_str(cart.header_checksum().to_string().as_str());
        ident.push('-');
        ident.push_str(cart.global_checksum().to_string().as_str());

        Ok(ident)
    }

    fn cart_from_path(path: &Path) -> anyhow::Result<ceres_core::Cart> {
        let rom = std::fs::read(path)
            .map(Vec::into_boxed_slice)
            .map_err(|e| anyhow::anyhow!(e))?;

        ceres_core::Cart::new(rom).map_err(|e| e.into())
    }

    fn ram_from_dirs_ident(ident: &str) -> anyhow::Result<Box<[u8]>> {
        let directories = directories::ProjectDirs::from(
            crate::QUALIFIER,
            crate::ORGANIZATION,
            crate::CERES_STYLIZED,
        )
        .unwrap();

        let path = directories.data_dir().join(ident).with_extension("sav");

        println!("Loading RAM from {:?}", path);

        std::fs::read(path)
            .map(Vec::into_boxed_slice)
            .map_err(|e| anyhow::anyhow!(e))
    }

    fn gb_loop(
        gb: Arc<Mutex<Gb<ceres_audio::RingBuffer>>>,
        exiting: Arc<AtomicBool>,
        pause_thread: Arc<AtomicBool>,
    ) {
        loop {
            let begin = std::time::Instant::now();

            if exiting.load(Relaxed) {
                break;
            }

            let duration = std::time::Duration::from_millis(1000 / 60);

            if !pause_thread.load(Relaxed) {
                if let Ok(mut gb) = gb.lock() {
                    gb.run_frame();
                }
            }

            let elapsed = begin.elapsed();

            if elapsed < duration {
                spin_sleep::sleep(duration - elapsed);
            }
            // TODO: we're always running late
            // else {
            //     eprintln!("running late: {elapsed:?}");
            // }
        }

        // FIXME: clippy says we have to drop
        drop(gb);
        drop(exiting);
        drop(pause_thread);
    }

    pub fn save_data(&self) {
        if let Ok(gb) = self.scene.gb().lock() {
            if let Some(save_data) = gb.cartridge().save_data() {
                // FIXME: don't repeat this everywhere
                let directories = directories::ProjectDirs::from(
                    crate::QUALIFIER,
                    crate::ORGANIZATION,
                    crate::CERES_STYLIZED,
                )
                .unwrap();

                std::fs::create_dir_all(directories.data_dir())
                    .expect("couldn't create data directory");

                let path = directories
                    .data_dir()
                    .join(&self.rom_ident)
                    .with_extension("sav");

                println!("Saving RAM to {:?}", path);

                let sav_file = std::fs::File::create(path);
                match sav_file {
                    Ok(mut f) => {
                        if let Err(e) = std::io::Write::write_all(&mut f, save_data) {
                            eprintln!("couldn't save data in save file: {e}");
                        }
                    }
                    Err(e) => {
                        eprintln!("couldn't open save file: {e}");
                    }
                }
            }
        }
    }
}

impl Drop for GbArea {
    fn drop(&mut self) {
        self.exiting.store(true, Relaxed);
        self.thread_handle.take().unwrap().join().unwrap();
        self.save_data();
    }
}
