package com.github.remind_me_later.ceres;


import android.util.Log;
import android.view.Surface;

public class RustBridge {
    public static native long createEmulator();

    public static native void renderFrame(long rustObj);

    public static native void dropWgpuState(long rustObj);

    public static native void recreateWgpuState(long rustObj, Surface surface);

    public static native void resizeWgpuState(long rustObj, int width, int height);

    public static void init() {
        System.loadLibrary("ceres_jni");
        Log.d("RustBridge", "Library loaded");
    }
}