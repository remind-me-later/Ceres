package com.github.remind_me_later.ceres

import android.app.Application
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.launch

sealed class EmulatorCommand {
    object SaveRAM : EmulatorCommand()
    object Pause : EmulatorCommand()
    object Resume : EmulatorCommand()
    data class SetSpeed(val multiplier: Int) : EmulatorCommand()
    object ToggleMute : EmulatorCommand()
    object Cleanup : EmulatorCommand()
}

class EmulatorViewModel(application: Application) : AndroidViewModel(application) {
    private val _emulatorCommands = MutableSharedFlow<EmulatorCommand>()
    val emulatorCommands = _emulatorCommands.asSharedFlow()

    override fun onCleared() {
        super.onCleared()
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.Cleanup)
        }
    }

    fun saveRAM() {
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.SaveRAM)
        }
    }

    fun pause() {
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.Pause)
        }
    }

    fun resume() {
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.Resume)
        }
    }

    fun setSpeed(multiplier: Int) {
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.SetSpeed(multiplier))
        }
    }

    fun toggleMute() {
        viewModelScope.launch {
            _emulatorCommands.emit(EmulatorCommand.ToggleMute)
        }
    }
}
