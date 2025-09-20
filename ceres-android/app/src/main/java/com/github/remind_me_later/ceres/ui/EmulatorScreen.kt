package com.github.remind_me_later.ceres.ui

import android.app.Activity
import android.net.Uri
import android.util.Log
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import com.github.remind_me_later.ceres.EmulatorSurfaceView
import com.github.remind_me_later.ceres.MainActivity
import com.github.remind_me_later.ceres.RustBridge
import com.github.remind_me_later.ceres.viewmodel.EmulatorCommand
import com.github.remind_me_later.ceres.viewmodel.EmulatorViewModel
import kotlinx.coroutines.flow.collectLatest

@Composable
fun EmulatorScreen(
    emulatorViewModel: EmulatorViewModel,
    romUri: Uri,
    onExit: () -> Unit,
    isNewSelection: Boolean,
    onRomLoaded: () -> Unit
) {
    val context = LocalActivity.current as MainActivity // Assuming MainActivity context
    val view = LocalView.current
    val window = (view.context as Activity).window
    val lifecycleOwner = androidx.lifecycle.compose.LocalLifecycleOwner.current

    // Create and manage EmulatorSurfaceView instance here
    val emulatorSurfaceView = remember {
        EmulatorSurfaceView(context).also {
            context.currentEmulatorSurfaceView = it // Set the global reference
        }
    }

    DisposableEffect(emulatorSurfaceView, lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> emulatorSurfaceView.resume() // Resume rendering when screen is visible
                Lifecycle.Event.ON_PAUSE -> emulatorSurfaceView.pause()   // Pause rendering when screen is not visible
                Lifecycle.Event.ON_DESTROY -> {
                    emulatorSurfaceView.cleanup() // Clean up when the composable is destroyed
                    if (context.currentEmulatorSurfaceView == emulatorSurfaceView) {
                        context.currentEmulatorSurfaceView = null // Clear the global reference
                    }
                }

                else -> {}
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)

        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            // Ensure cleanup if not already done by ON_DESTROY (e.g. quick exit)
            // emulatorSurfaceView.cleanup() // This might be redundant if ON_DESTROY always fires
            if (context.currentEmulatorSurfaceView == emulatorSurfaceView) {
                context.currentEmulatorSurfaceView = null
            }
        }
    }


    LaunchedEffect(emulatorViewModel.emulatorCommands, emulatorSurfaceView) {
        emulatorViewModel.emulatorCommands.collectLatest {
            when (it) {
                EmulatorCommand.SaveRAM -> emulatorSurfaceView.saveRAM()
                EmulatorCommand.Pause -> emulatorSurfaceView.pause()
                EmulatorCommand.Resume -> emulatorSurfaceView.resume()
                is EmulatorCommand.SetSpeed -> RustBridge.setSpeedMultiplier(
                    emulatorSurfaceView.emulatorPtr, it.multiplier
                )

                EmulatorCommand.ToggleMute -> RustBridge.toggleMute(emulatorSurfaceView.emulatorPtr)

                is EmulatorCommand.SetColorCorrectionMode -> RustBridge.setColorCorrectionMode(
                    emulatorSurfaceView.emulatorPtr, it.mode
                )

                is EmulatorCommand.SetShaderOption -> RustBridge.setShaderOption(
                    emulatorSurfaceView.emulatorPtr, it.shader
                )

                EmulatorCommand.Cleanup -> emulatorSurfaceView.cleanup()
            }
        }
    }

    DisposableEffect(Unit) {
        val insetsController = WindowCompat.getInsetsController(window, view)
        insetsController.hide(WindowInsetsCompat.Type.statusBars())
        insetsController.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE

        // Resume emulator when entering this screen
        emulatorViewModel.resume() // Or directly emulatorSurfaceView.resume() if preferred

        onDispose {
            insetsController.show(WindowInsetsCompat.Type.statusBars())
            // Pausing is now handled by lifecycle observer or onExit lambda
        }
    }

    BackHandler {
        onExit() // This will trigger pause and saveRAM via the onExit lambda in CeresApp
    }

    LaunchedEffect(isNewSelection) {
        if (isNewSelection) {
            context.loadRomFromUri(emulatorSurfaceView, romUri)
            onRomLoaded() // Signal that the new ROM has been processed
        }
    }

    Box(modifier = Modifier.fillMaxSize()) {
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxSize()
                .background(Color.Black),
            contentAlignment = Alignment.Center
        ) {
            val boxWidth = this.maxWidth
            val boxHeight = this.maxHeight
            val aspectRatio = 160f / 144f

            val modifier = if (boxWidth / boxHeight > aspectRatio) {
                Modifier
                    .fillMaxHeight()
                    .aspectRatio(aspectRatio)
            } else {
                Modifier
                    .fillMaxWidth()
                    .aspectRatio(aspectRatio)
            }

            AndroidView(
                factory = { emulatorSurfaceView }, // Use the local instance
                modifier = modifier, update = { view ->
                    // Optional: If you need to tell the view it's being reused / reattached
                    // For instance, if surfaceCreated wasn't being called reliably on return to screen
                    Log.d("EmulatorScreen", "AndroidView update for: ${view.hashCode()}")
                })
        }

        GameBoyControls(
            onButtonPress = { buttonId ->
                if (emulatorSurfaceView.emulatorPtr != 0L) {
                    RustBridge.pressButton(emulatorSurfaceView.emulatorPtr, buttonId)
                }
            }, onButtonRelease = { buttonId ->
                if (emulatorSurfaceView.emulatorPtr != 0L) {
                    RustBridge.releaseButton(emulatorSurfaceView.emulatorPtr, buttonId)
                }
            }, modifier = Modifier
                .align(Alignment.BottomCenter)
                .fillMaxWidth()
                .padding(16.dp)
        )
    }
}
