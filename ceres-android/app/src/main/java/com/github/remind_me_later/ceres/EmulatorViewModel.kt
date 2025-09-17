package com.github.remind_me_later.ceres

import android.app.Application
import androidx.lifecycle.AndroidViewModel

class EmulatorViewModel(application: Application) : AndroidViewModel(application) {
    val emulatorSurfaceView = EmulatorSurfaceView(application)

    override fun onCleared() {
        emulatorSurfaceView.cleanup()
        super.onCleared()
    }

    fun saveRAM() {
        emulatorSurfaceView.saveRAM()
    }

    fun pause() {
        emulatorSurfaceView.pause()
    }

    fun resume() {
        emulatorSurfaceView.resume()
    }
}
