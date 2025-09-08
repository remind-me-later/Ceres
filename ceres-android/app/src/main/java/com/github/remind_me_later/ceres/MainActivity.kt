package com.github.remind_me_later.ceres

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.*
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import kotlinx.coroutines.launch

class MainActivity : ComponentActivity() {
    private var emulatorSurfaceView: EmulatorSurfaceView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background
                ) {
                    EmulatorScreen { surfaceView ->
                        emulatorSurfaceView = surfaceView
                    }
                }
            }
        }
    }

    fun loadRomFromUri(uri: Uri) {
        emulatorSurfaceView?.let { surfaceView ->
            try {
                Log.d("MainActivity", "Loading ROM from URI: $uri")

                // Copy the content URI to a temporary file
                val tempFile = copyUriToTempFile(uri)
                if (tempFile != null) {
                    val success =
                        RustBridge.loadRom(surfaceView.emulatorPtr, tempFile.absolutePath, null)
                    if (success) {
                        Log.d("MainActivity", "ROM loaded successfully: ${tempFile.absolutePath}")
                    } else {
                        Log.e("MainActivity", "Failed to load ROM: ${tempFile.absolutePath}")
                    }
                } else {
                    Log.e("MainActivity", "Failed to copy ROM file")
                }
            } catch (e: Exception) {
                Log.e("MainActivity", "Error loading ROM: ${e.message}")
            }
        }
    }

    private fun copyUriToTempFile(uri: Uri): File? {
        return try {
            val inputStream: InputStream? = contentResolver.openInputStream(uri)
            if (inputStream == null) {
                Log.e("MainActivity", "Could not open input stream for URI: $uri")
                return null
            }

            // Get the file name from the URI
            val fileName = getFileNameFromUri(uri) ?: "temp_rom.gb"

            // Sanitize filename to prevent path traversal issues
            val sanitizedFileName = fileName.replace(Regex("[^a-zA-Z0-9._-]"), "_")

            // Create a temporary file in the app's cache directory
            val tempFile = File(cacheDir, "roms/$sanitizedFileName")
            tempFile.parentFile?.mkdirs() // Create directories if they don't exist

            // Delete existing file if it exists
            if (tempFile.exists()) {
                tempFile.delete()
            }

            Log.d("MainActivity", "Copying ROM to: ${tempFile.absolutePath}")

            // Copy the input stream to the temporary file
            val outputStream = FileOutputStream(tempFile)
            inputStream.use { input ->
                outputStream.use { output ->
                    val buffer = ByteArray(8192) // 8KB buffer for efficiency
                    var bytesRead: Int
                    var totalBytes = 0L
                    while (input.read(buffer).also { bytesRead = it } != -1) {
                        output.write(buffer, 0, bytesRead)
                        totalBytes += bytesRead
                    }
                    Log.d("MainActivity", "ROM copied successfully, size: $totalBytes bytes")
                }
            }

            // Verify the file was created and has content
            if (tempFile.exists() && tempFile.length() > 0) {
                tempFile
            } else {
                Log.e("MainActivity", "Copied file is empty or doesn't exist")
                null
            }
        } catch (e: Exception) {
            Log.e("MainActivity", "Error copying ROM file: ${e.message}", e)
            null
        }
    }

    fun getFileNameFromUri(uri: Uri): String? {
        return try {
            val cursor = contentResolver.query(uri, null, null, null, null)
            cursor?.use {
                if (it.moveToFirst()) {
                    val displayNameIndex =
                        it.getColumnIndex(android.provider.OpenableColumns.DISPLAY_NAME)
                    if (displayNameIndex >= 0) {
                        return it.getString(displayNameIndex)
                    }
                }
            }

            // Fallback to the last path segment
            uri.lastPathSegment
        } catch (e: Exception) {
            Log.e("MainActivity", "Error getting file name: ${e.message}")
            uri.lastPathSegment
        }
    }

    override fun onDestroy() {
        emulatorSurfaceView?.cleanup()
        cleanupTempRomFiles()
        super.onDestroy()
    }

    private fun cleanupTempRomFiles() {
        try {
            val romsDir = File(cacheDir, "roms")
            if (romsDir.exists()) {
                romsDir.listFiles()?.forEach { file ->
                    if (file.isFile) {
                        file.delete()
                        Log.d("MainActivity", "Cleaned up temp ROM file: ${file.name}")
                    }
                }
            }
        } catch (e: Exception) {
            Log.e("MainActivity", "Error cleaning up temp ROM files: ${e.message}")
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EmulatorScreen(onSurfaceViewCreated: (EmulatorSurfaceView) -> Unit) {
    val screenWidth = LocalConfiguration.current.screenWidthDp.dp
    var showMenu by remember { mutableStateOf(false) }
    var emulatorSurfaceView by remember { mutableStateOf<EmulatorSurfaceView?>(null) }
    var isPaused by remember { mutableStateOf(false) }
    var currentRomName by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    val context = androidx.compose.ui.platform.LocalContext.current

    // File picker launcher
    val filePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.GetContent()
    ) { uri ->
        uri?.let { selectedUri ->
            // Get the activity context to call loadRomFromUri
            if (context is MainActivity) {
                context.loadRomFromUri(selectedUri)
                currentRomName = context.getFileNameFromUri(selectedUri)
            }
        }
    }

    Column(
        modifier = Modifier.fillMaxSize()
    ) {
        // Top App Bar with Menu
        TopAppBar(title = {
            Text(
                text = currentRomName ?: "Ceres Game Boy Emulator",
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold
            )
        }, actions = {
            // Pause/Resume button
            emulatorSurfaceView?.let { surfaceView ->
                IconButton(
                    onClick = {
                        scope.launch {
                            val success = if (isPaused) {
                                RustBridge.resumeEmulator(surfaceView.emulatorPtr)
                            } else {
                                RustBridge.pauseEmulator(surfaceView.emulatorPtr)
                            }
                            if (success) {
                                isPaused = !isPaused
                            }
                        }
                    }) {
                    Icon(
                        imageVector = if (isPaused) Icons.Default.PlayArrow else Icons.Default.Done,
                        contentDescription = if (isPaused) "Resume" else "Pause"
                    )
                }
            }

            // Menu button
            IconButton(onClick = { showMenu = true }) {
                Icon(Icons.Default.Menu, contentDescription = "Menu")
            }

            // Dropdown menu
            DropdownMenu(
                expanded = showMenu, onDismissRequest = { showMenu = false }) {
                DropdownMenuItem(text = { Text("Open ROM") }, onClick = {
                    showMenu = false
                    // Launch file picker for any file type (Android doesn't support .gb/.gbc MIME types well)
                    filePickerLauncher.launch("*/*")
                })

                emulatorSurfaceView?.let { surfaceView ->
                    DropdownMenuItem(text = { Text("Speed 1x") }, onClick = {
                        showMenu = false
                        RustBridge.setSpeedMultiplier(surfaceView.emulatorPtr, 1)
                    })
                    DropdownMenuItem(text = { Text("Speed 2x") }, onClick = {
                        showMenu = false
                        RustBridge.setSpeedMultiplier(surfaceView.emulatorPtr, 2)
                    })
                    DropdownMenuItem(text = { Text("Speed 4x") }, onClick = {
                        showMenu = false
                        RustBridge.setSpeedMultiplier(surfaceView.emulatorPtr, 4)
                    })

                    Divider()

                    DropdownMenuItem(text = { Text("Toggle Mute") }, onClick = {
                        showMenu = false
                        RustBridge.toggleMute(surfaceView.emulatorPtr)
                    })
                }
            }
        })

        // Emulator Surface
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f), contentAlignment = Alignment.Center
        ) {
            AndroidView(
                factory = { context ->
                    EmulatorSurfaceView(context).also { surfaceView ->
                        emulatorSurfaceView = surfaceView
                        onSurfaceViewCreated(surfaceView)
                    }
                },
                modifier = Modifier
                    .fillMaxWidth()
                    .aspectRatio(160f / 144f) // Game Boy screen aspect ratio
            )
        }

        // Virtual Game Boy Controls
        GameBoyControls(
            onButtonPress = { buttonId ->
            emulatorSurfaceView?.let { surfaceView ->
                RustBridge.pressButton(surfaceView.emulatorPtr, buttonId)
            }
        }, onButtonRelease = { buttonId ->
            emulatorSurfaceView?.let { surfaceView ->
                RustBridge.releaseButton(surfaceView.emulatorPtr, buttonId)
            }
        }, modifier = Modifier
                .fillMaxWidth()
                .padding(16.dp)
        )
    }
}

@Composable
fun GameBoyControls(
    onButtonPress: (Int) -> Unit, onButtonRelease: (Int) -> Unit, modifier: Modifier = Modifier
) {
    Row(
        modifier = modifier,
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically
    ) {
        // D-Pad
        Column(
            horizontalAlignment = Alignment.CenterHorizontally
        ) {
            GameBoyButton(
                text = "↑",
                buttonId = RustBridge.BUTTON_UP,
                onPress = onButtonPress,
                onRelease = onButtonRelease
            )
            Row {
                GameBoyButton(
                    text = "←",
                    buttonId = RustBridge.BUTTON_LEFT,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
                Spacer(modifier = Modifier.width(48.dp))
                GameBoyButton(
                    text = "→",
                    buttonId = RustBridge.BUTTON_RIGHT,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
            }
            GameBoyButton(
                text = "↓",
                buttonId = RustBridge.BUTTON_DOWN,
                onPress = onButtonPress,
                onRelease = onButtonRelease
            )
        }

        // Action Buttons
        Column {
            Row {
                GameBoyButton(
                    text = "SELECT",
                    buttonId = RustBridge.BUTTON_SELECT,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
                Spacer(modifier = Modifier.width(8.dp))
                GameBoyButton(
                    text = "START",
                    buttonId = RustBridge.BUTTON_START,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
            }
            Spacer(modifier = Modifier.height(16.dp))
            Row {
                GameBoyButton(
                    text = "B",
                    buttonId = RustBridge.BUTTON_B,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
                Spacer(modifier = Modifier.width(8.dp))
                GameBoyButton(
                    text = "A",
                    buttonId = RustBridge.BUTTON_A,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease
                )
            }
        }
    }
}

@Composable
fun GameBoyButton(
    text: String,
    buttonId: Int,
    onPress: (Int) -> Unit,
    onRelease: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    var isPressed by remember { mutableStateOf(false) }
    val interactionSource = remember { MutableInteractionSource() }

    LaunchedEffect(interactionSource) {
        interactionSource.interactions.collect { interaction ->
            when (interaction) {
                is PressInteraction.Press -> {
                    isPressed = true
                    onPress(buttonId)
                }

                is PressInteraction.Release -> {
                    isPressed = false
                    onRelease(buttonId)
                }

                is PressInteraction.Cancel -> {
                    isPressed = false
                    onRelease(buttonId)
                }
            }
        }
    }

    Button(
        onClick = { }, // Interaction handled by interactionSource
        modifier = modifier.size(48.dp),
        interactionSource = interactionSource,
        colors = ButtonDefaults.buttonColors(
            containerColor = if (isPressed) MaterialTheme.colorScheme.primaryContainer else MaterialTheme.colorScheme.primary
        )
    ) {
        Text(
            text = text, fontSize = 10.sp, fontWeight = FontWeight.Bold
        )
    }
}