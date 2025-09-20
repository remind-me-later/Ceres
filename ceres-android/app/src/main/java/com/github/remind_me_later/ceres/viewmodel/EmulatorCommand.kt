package com.github.remind_me_later.ceres.viewmodel

sealed class EmulatorCommand {
    object SaveRAM : EmulatorCommand()
    object Pause : EmulatorCommand()
    object Resume : EmulatorCommand()
    data class SetSpeed(val multiplier: Int) : EmulatorCommand()
    object ToggleMute : EmulatorCommand()
    data class SetColorCorrectionMode(val mode: Int) : EmulatorCommand()
    data class SetShaderOption(val shader: Int) : EmulatorCommand()
    object Cleanup : EmulatorCommand()
}
