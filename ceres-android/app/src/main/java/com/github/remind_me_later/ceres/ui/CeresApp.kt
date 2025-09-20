package com.github.remind_me_later.ceres.ui

import android.net.Uri
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewmodel.compose.viewModel
import com.github.remind_me_later.ceres.viewmodel.EmulatorViewModel

enum class Screen {
    RomList, Emulator
}

@Composable
fun CeresApp() {
    val emulatorViewModel: EmulatorViewModel = viewModel()
    var currentScreen by remember { mutableStateOf(Screen.RomList) }
    var selectedRom by remember { mutableStateOf<Uri?>(null) }
    var isNewSelection by remember { mutableStateOf(false) }

    val isGameRunning = selectedRom != null

    when (currentScreen) {
        Screen.RomList -> {
            RomListScreen(emulatorViewModel = emulatorViewModel, onRomSelected = { romUri ->
                if (selectedRom != romUri) {
                    selectedRom = romUri
                }
                isNewSelection = true // Flag that a new ROM is selected
                currentScreen = Screen.Emulator
            }, isGameRunning = isGameRunning, onReturnToGame = {
                isNewSelection = false // Returning to existing game, not a new selection
                currentScreen = Screen.Emulator
            })
        }

        Screen.Emulator -> {
            EmulatorScreen(
                emulatorViewModel = emulatorViewModel,
                romUri = selectedRom!!, // Non-null asserted as it's set before navigating here
                onExit = {
                    // When exiting emulator, always tell ViewModel to pause and save.
                    emulatorViewModel.pause()
                    emulatorViewModel.saveRAM()
                    currentScreen = Screen.RomList
                },
                isNewSelection = isNewSelection,
                onRomLoaded = { isNewSelection = false } // Reset flag after new ROM is processed
            )
        }
    }
}
