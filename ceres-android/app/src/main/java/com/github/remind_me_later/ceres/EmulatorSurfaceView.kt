package com.github.remind_me_later.ceres

import android.content.Context
import android.graphics.Canvas
import android.graphics.PixelFormat
import android.util.AttributeSet
import android.util.Log
import android.view.SurfaceHolder
import android.view.SurfaceView

class EmulatorSurfaceView : SurfaceView, SurfaceHolder.Callback2 {
    var emulatorPtr: Long = 0
        private set

    private var isRenderingActive = false

    constructor(context: Context) : super(context) {
        init()
    }

    constructor(context: Context, attrs: AttributeSet) : super(context, attrs) {
        init()
    }

    constructor(
        context: Context, attrs: AttributeSet, defStyle: Int
    ) : super(context, attrs, defStyle) {
        init()
    }

    private fun init() {
        RustBridge.init()
        emulatorPtr = RustBridge.createEmulator()
        RustBridge.resumeEmulator(emulatorPtr)
        holder.addCallback(this)
        // Set up the surface for transparent rendering
        this.setZOrderMediaOverlay(true)
        holder.setFormat(PixelFormat.TRANSLUCENT)
        Log.d("EmulatorSurfaceView", "Surface view initialized")
    }

    override fun surfaceChanged(holder: SurfaceHolder, format: Int, width: Int, height: Int) {
        RustBridge.resizeWgpuState(this.emulatorPtr, width, height)
    }

    override fun surfaceCreated(holder: SurfaceHolder) {
        Log.d("EmulatorSurfaceView", "Surface created")
        holder.let { h ->
            try {
                isRenderingActive = true
                setWillNotDraw(false)

                // Start the render loop
                invalidate()

                RustBridge.recreateWgpuState(this.emulatorPtr, h.surface)
                Log.d("EmulatorSurfaceView", "Emulator created with pointer: $emulatorPtr")
            } catch (e: Exception) {
                Log.e("EmulatorSurfaceView", "Failed to create emulator", e)
            }
        }
    }

    override fun surfaceDestroyed(holder: SurfaceHolder) {
        Log.d("EmulatorSurfaceView", "Surface destroyed")
        isRenderingActive = false

        if (emulatorPtr != 0L) {
            try {
                RustBridge.dropWgpuState(emulatorPtr)
                Log.d("EmulatorSurfaceView", "Wgpu state dropped")
            } catch (e: Exception) {
                Log.e("EmulatorSurfaceView", "Failed to drop wgpu state", e)
            }
        }
    }

    override fun surfaceRedrawNeeded(holder: SurfaceHolder) {
        Log.d("EmulatorSurfaceView", "Surface redraw needed")
        if (isRenderingActive && emulatorPtr != 0L) {
            renderFrame()
        }
    }

    override fun surfaceRedrawNeededAsync(holder: SurfaceHolder, drawingFinished: Runnable) {
        super.surfaceRedrawNeededAsync(holder, drawingFinished)
        if (isRenderingActive && emulatorPtr != 0L) {
            renderFrame()
        }
        drawingFinished.run()
    }

    override fun onDraw(canvas: Canvas) {
        super.onDraw(canvas)
        if (isRenderingActive && emulatorPtr != 0L) {
            renderFrame()
            // Continue the render loop
            invalidate()
        }
    }

    private fun renderFrame() {
        try {
            RustBridge.renderFrame(emulatorPtr)
        } catch (e: Exception) {
            Log.e("EmulatorSurfaceView", "Failed to render frame", e)
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        isRenderingActive = false
        RustBridge.dropWgpuState(emulatorPtr)
        Log.d("EmulatorSurfaceView", "Detached from window")
    }

    fun cleanup() {
        if (emulatorPtr != 0L) {
            try {
                RustBridge.destroyEmulator(emulatorPtr)
                emulatorPtr = 0L
                Log.d("EmulatorSurfaceView", "Emulator destroyed")
            } catch (e: Exception) {
                Log.e("EmulatorSurfaceView", "Failed to destroy emulator", e)
            }
        }
    }
}
