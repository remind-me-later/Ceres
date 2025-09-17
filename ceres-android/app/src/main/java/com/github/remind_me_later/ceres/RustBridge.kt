package com.github.remind_me_later.ceres

import android.util.Log
import android.view.Surface
import java.io.IOException

object RustBridge {
    /**
     * Creates a new emulator instance.
     *
     * @return Pointer to the emulator instance, or 0 if creation failed
     * @throws RuntimeException if emulator creation fails
     */
    @Throws(RuntimeException::class)
    external fun createEmulator(): Long

    /**
     * Renders a single frame.
     *
     * @param rustObj Pointer to the emulator instance
     * @throws IllegalArgumentException if rustObj is invalid
     * @throws RuntimeException if rendering fails
     */
    @Throws(IllegalArgumentException::class, RuntimeException::class)
    external fun renderFrame(rustObj: Long)

    /**
     * Drops the WGPU state.
     *
     * @param rustObj Pointer to the emulator instance
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun dropWgpuState(rustObj: Long)

    /**
     * Recreates the WGPU state with a new surface.
     *
     * @param rustObj Pointer to the emulator instance
     * @param surface Android surface for rendering
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun recreateWgpuState(rustObj: Long, surface: Surface?)

    /**
     * Resizes the WGPU state.
     *
     * @param rustObj Pointer to the emulator instance
     * @param width New width
     * @param height New height
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun resizeWgpuState(rustObj: Long, width: Int, height: Int)

    /**
     * Loads a ROM file into the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @param romPath Path to the ROM file
     * @param savPath Path to the save file (can be null)
     * @return true if the ROM was loaded successfully, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid or string conversion fails
     * @throws IOException if ROM loading fails
     */
    @Throws(IllegalArgumentException::class, IOException::class)
    external fun loadRom(rustObj: Long, romPath: String?, savPath: String?): Boolean

    /**
     * Presses a button on the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @param buttonId ID of the button to press
     * @throws IllegalArgumentException if rustObj is invalid or buttonId is out of range
     */
    @Throws(IllegalArgumentException::class)
    external fun pressButton(rustObj: Long, buttonId: Int)

    /**
     * Releases a button on the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @param buttonId ID of the button to release
     * @throws IllegalArgumentException if rustObj is invalid or buttonId is out of range
     */
    @Throws(IllegalArgumentException::class)
    external fun releaseButton(rustObj: Long, buttonId: Int)

    /**
     * Pauses the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @return true if successfully paused, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid
     * @throws IllegalStateException if pause operation fails
     */
    @Throws(IllegalArgumentException::class, IllegalStateException::class)
    external fun pauseEmulator(rustObj: Long): Boolean

    /**
     * Resumes the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @return true if successfully resumed, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid
     * @throws IllegalStateException if resume operation fails
     */
    @Throws(IllegalArgumentException::class, IllegalStateException::class)
    external fun resumeEmulator(rustObj: Long): Boolean

    /**
     * Checks if the emulator is paused.
     *
     * @param rustObj Pointer to the emulator instance
     * @return true if paused, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun isPaused(rustObj: Long): Boolean

    /**
     * Sets the emulator speed multiplier.
     *
     * @param rustObj Pointer to the emulator instance
     * @param multiplier Speed multiplier
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun setSpeedMultiplier(rustObj: Long, multiplier: Int)

    /**
     * Sets the emulator volume.
     *
     * @param rustObj Pointer to the emulator instance
     * @param volume Volume level (0.0 to 1.0)
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun setVolume(rustObj: Long, volume: Float)

    /**
     * Toggles mute on the emulator.
     *
     * @param rustObj Pointer to the emulator instance
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun toggleMute(rustObj: Long)

    /**
     * Destroys the emulator instance and frees memory.
     *
     * @param rustObj Pointer to the emulator instance
     */
    external fun destroyEmulator(rustObj: Long)

    /**
     * Handles WGPU lost state.
     *
     * @param rustObj Pointer to the emulator instance
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun onWgpuLost(rustObj: Long)

    /**
     * Saves the emulator's RAM to a file.
     *
     * @param rustObj Pointer to the emulator instance
     * @param path Path to save the RAM file
     * @throws IllegalArgumentException if rustObj is invalid
     * @throws IOException if saving fails
     */
    @Throws(IllegalArgumentException::class, IOException::class)
    external fun saveRam(rustObj: Long, path: String)

    /**
     * Sets the color correction mode.
     *
     * @param rustObj Pointer to the emulator instance
     * @param mode Color correction mode (0-5)
     * @throws IllegalArgumentException if rustObj is invalid or mode is out of range
     */
    @Throws(IllegalArgumentException::class)
    external fun setColorCorrectionMode(rustObj: Long, mode: Int)

    /**
     * Sets the shader/scaling option.
     *
     * @param rustObj Pointer to the emulator instance
     * @param shader Shader option (0-4)
     * @throws IllegalArgumentException if rustObj is invalid or shader is out of range
     */
    @Throws(IllegalArgumentException::class)
    external fun setShaderOption(rustObj: Long, shader: Int)

    /**
     * Changes the Game Boy model.
     *
     * @param rustObj Pointer to the emulator instance
     * @param model Model type (0-2)
     * @throws IllegalArgumentException if rustObj is invalid or model is out of range
     */
    @Throws(IllegalArgumentException::class)
    external fun changeModel(rustObj: Long, model: Int)

    /**
     * Gets the current Game Boy model.
     *
     * @param rustObj Pointer to the emulator instance
     * @return Current model (0-2), or -1 if invalid
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun getModel(rustObj: Long): Int

    /**
     * Gets the current speed multiplier.
     *
     * @param rustObj Pointer to the emulator instance
     * @return Current speed multiplier
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun getSpeedMultiplier(rustObj: Long): Int

    /**
     * Checks if the emulator is muted.
     *
     * @param rustObj Pointer to the emulator instance
     * @return true if muted, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun isMuted(rustObj: Long): Boolean

    /**
     * Gets the current volume level.
     *
     * @param rustObj Pointer to the emulator instance
     * @return Current volume (0.0 to 1.0)
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun getVolume(rustObj: Long): Float

    /**
     * Checks if the cartridge has save data.
     *
     * @param rustObj Pointer to the emulator instance
     * @return true if save data exists, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun hasSaveData(rustObj: Long): Boolean

    /**
     * Activates a Game Genie code.
     * Note: Only available if compiled with game_genie feature.
     *
     * @param rustObj Pointer to the emulator instance
     * @param code Game Genie code string (format: "ABC-DEF-GHI")
     * @return true if activated successfully, false otherwise
     * @throws IllegalArgumentException if rustObj is invalid or code format is invalid
     * @throws RuntimeException if activation fails
     */
    @Throws(IllegalArgumentException::class, RuntimeException::class)
    external fun activateGameGenie(rustObj: Long, code: String): Boolean

    /**
     * Deactivates a Game Genie code.
     * Note: Only available if compiled with game_genie feature.
     *
     * @param rustObj Pointer to the emulator instance
     * @param code Game Genie code string to deactivate
     * @throws IllegalArgumentException if rustObj is invalid or code format is invalid
     */
    @Throws(IllegalArgumentException::class)
    external fun deactivateGameGenie(rustObj: Long, code: String)

    /**
     * Gets all currently active Game Genie codes.
     * Note: Only available if compiled with game_genie feature.
     *
     * @param rustObj Pointer to the emulator instance
     * @return Array of active Game Genie code strings
     * @throws IllegalArgumentException if rustObj is invalid
     * @throws RuntimeException if retrieval fails
     */
    @Throws(IllegalArgumentException::class, RuntimeException::class)
    external fun getActiveGameGenieCodes(rustObj: Long): Array<String>

    // Color correction mode constants
    const val COLOR_CORRECT_CURVES: Int = 0
    const val COLOR_DISABLED: Int = 1
    const val COLOR_LOW_CONTRAST: Int = 2
    const val COLOR_MODERN_BALANCED: Int = 3
    const val COLOR_MODERN_BOOST_CONTRAST: Int = 4
    const val COLOR_REDUCE_CONTRAST: Int = 5

    // Shader/scaling option constants
    const val SHADER_CRT: Int = 0
    const val SHADER_LCD: Int = 1
    const val SHADER_NEAREST: Int = 2
    const val SHADER_SCALE2X: Int = 3
    const val SHADER_SCALE3X: Int = 4

    // Game Boy model constants
    const val MODEL_DMG: Int = 0  // Original Game Boy
    const val MODEL_CGB: Int = 1  // Game Boy Color
    const val MODEL_MGB: Int = 2  // Game Boy Pocket

    // Button constants
    const val BUTTON_RIGHT: Int = 0
    const val BUTTON_LEFT: Int = 1
    const val BUTTON_UP: Int = 2
    const val BUTTON_DOWN: Int = 3
    const val BUTTON_A: Int = 4
    const val BUTTON_B: Int = 5
    const val BUTTON_SELECT: Int = 6
    const val BUTTON_START: Int = 7

    fun init() {
        System.loadLibrary("ceres_jni")
        Log.d("RustBridge", "Library loaded")
    }
}
