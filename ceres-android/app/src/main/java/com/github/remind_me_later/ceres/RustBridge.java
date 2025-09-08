package com.github.remind_me_later.ceres;


import android.util.Log;
import android.view.Surface;

public class RustBridge {
    public static native long createEmulator();

    public static native void renderFrame(long rustObj);

    public static native void dropWgpuState(long rustObj);

    public static native void recreateWgpuState(long rustObj, Surface surface);

    public static native void resizeWgpuState(long rustObj, int width, int height);

    public static native boolean loadRom(long rustObj, String romPath, String savPath);

    public static native void pressButton(long rustObj, int buttonId);

    public static native void releaseButton(long rustObj, int buttonId);

    public static native boolean pauseEmulator(long rustObj);

    public static native boolean resumeEmulator(long rustObj);

    public static native boolean isPaused(long rustObj);

    public static native void setSpeedMultiplier(long rustObj, int multiplier);

    public static native void setVolume(long rustObj, float volume);

    public static native void toggleMute(long rustObj);

    public static native void destroyEmulator(long rustObj);

    // Additional method for handling wgpu lost state
    public static native void onWgpuLost(long rustObj);

    // Button constants matching the FFI implementation
    public static final int BUTTON_RIGHT = 0;
    public static final int BUTTON_LEFT = 1;
    public static final int BUTTON_UP = 2;
    public static final int BUTTON_DOWN = 3;
    public static final int BUTTON_A = 4;
    public static final int BUTTON_B = 5;
    public static final int BUTTON_SELECT = 6;
    public static final int BUTTON_START = 7;

    public static void init() {
        System.loadLibrary("ceres_jni");
        Log.d("RustBridge", "Library loaded");
    }
}