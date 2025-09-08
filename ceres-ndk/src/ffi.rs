use crate::emulator::Emulator;
use jni::objects::JClass;
use jni::sys::{JNIEnv, jlong, jobject};
use jni_fn::jni_fn;
use log::info;

#[unsafe(no_mangle)]
#[jni_fn("com.github.remind_me_later.ceres.RustBridge")]
pub fn createEmulator(env: *mut JNIEnv, _: JClass, surface: jobject) -> jlong {
    let emulator = Box::new(Emulator::new(env, surface));
    let ptr = Box::into_raw(emulator);
    info!("Emulator created at {:p}", ptr);
    ptr as jlong
}

#[unsafe(no_mangle)]
#[jni_fn("com.github.remind_me_later.ceres.RustBridge")]
pub fn renderFrame(emulator_ptr: jlong) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.render();
}

#[unsafe(no_mangle)]
#[jni_fn("com.github.remind_me_later.ceres.RustBridge")]
pub fn dropWgpuState(emulator_ptr: jlong) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.drop_state();
    info!("Dropped wgpu state");
}

#[unsafe(no_mangle)]
#[jni_fn("com.github.remind_me_later.ceres.RustBridge")]
pub fn recreateWgpuState(env: *mut JNIEnv, emulator_ptr: jlong, surface: jobject) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.recreate_state(env, surface);
    info!("Recreated wgpu state");
}
