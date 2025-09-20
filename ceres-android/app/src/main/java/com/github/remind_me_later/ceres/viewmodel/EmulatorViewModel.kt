package com.github.remind_me_later.ceres.viewmodel

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.launch

class EmulatorViewModel : ViewModel() {
    val emulatorCommands = MutableSharedFlow<EmulatorCommand>()

    fun setSpeed(speed: Int) {
        viewModelScope.launch {
            emulatorCommands.emit(EmulatorCommand.SetSpeed(speed))
        }
    }

    fun toggleMute() {
        viewModelScope.launch {
            emulatorCommands.emit(EmulatorCommand.ToggleMute)
        }
    }

    fun pause() {
        viewModelScope.launch {
            emulatorCommands.emit(EmulatorCommand.Pause)
        }
    }

    fun saveRAM() {
        viewModelScope.launch {
            emulatorCommands.emit(EmulatorCommand.SaveRAM)
        }
    }

    fun resume() {
        viewModelScope.launch {
            emulatorCommands.emit(EmulatorCommand.Resume)
        }
    }
}
