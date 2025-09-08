#![allow(non_snake_case)]

use crate::emulator::Emulator;
use jni::JNIEnv;
use jni::objects::{JClass, JObject};
use jni::sys::{jint, jlong};
use log::debug;

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_createEmulator(
    env: JNIEnv,
    _class: JClass,
) -> jlong {
    let emulator = Box::new(Emulator::new());
    let ptr = Box::into_raw(emulator);
    debug!("Emulator created at {:p}", ptr);
    ptr as jlong
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_renderFrame(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.render();
    debug!("Rendered frame");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_dropWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.drop_state();
    debug!("Dropped wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_recreateWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    surface: JObject,
) {
    debug!("Entering recreate_wgpu_state");
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    debug!("Recreating wgpu state");
    emulator.recreate_state(env.get_raw(), surface.as_raw());
    debug!("Recreated wgpu state");
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_resizeWgpuState(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
    width: jint,
    height: jint,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.resize(width as u32, height as u32);
    debug!("Resized wgpu state to {}x{}", width, height);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_com_github_remind_1me_1later_ceres_RustBridge_onWgpuLost(
    env: JNIEnv,
    _class: JClass,
    emulator_ptr: jlong,
) {
    let emulator = unsafe { &mut *(emulator_ptr as *mut Emulator) };
    emulator.on_lost();
    debug!("Handled wgpu lost");
}
