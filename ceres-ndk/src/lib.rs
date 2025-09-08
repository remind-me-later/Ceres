mod emulator;
mod ffi;
mod video;

pub use ffi::*;

#[unsafe(no_mangle)]
fn android_main(_app: ndk::native_activity::NativeActivity) {
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("CeresEmulator"),
    );

    log::info!("Ceres Android emulator started");
}
