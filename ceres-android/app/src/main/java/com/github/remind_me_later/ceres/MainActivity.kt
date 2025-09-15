package com.github.remind_me_later.ceres

import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.layout.*
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Done
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.*
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import kotlinx.coroutines.launch

private val DPadColor = Color(0xFF0E131A)
private val ActionButtonColor = Color(0xFF84254C)
private val StartSelectButtonColor = Color(0xFFC2B9AA)

class MainActivity : ComponentActivity() {
    private var emulatorSurfaceView: EmulatorSurfaceView? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(
                    modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background
                ) { EmulatorScreen { surfaceView -> emulatorSurfaceView = surfaceView } }
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
    var showMenu by remember { mutableStateOf(false) }
    var emulatorSurfaceView by remember { mutableStateOf<EmulatorSurfaceView?>(null) }
    var isPaused by remember { mutableStateOf(false) }
    var currentRomName by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    val context = androidx.compose.ui.platform.LocalContext.current

    // File picker launcher
    val filePickerLauncher =
        rememberLauncherForActivityResult(contract = ActivityResultContracts.GetContent()) { uri ->
            uri?.let { selectedUri ->
                // Get the activity context to call loadRomFromUri
                if (context is MainActivity) {
                    context.loadRomFromUri(selectedUri)
                    currentRomName = context.getFileNameFromUri(selectedUri)
                }
            }
        }

    Column(modifier = Modifier.fillMaxSize()) {
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
                                RustBridge.resumeEmulator(
                                    surfaceView.emulatorPtr
                                )
                            } else {
                                RustBridge.pauseEmulator(
                                    surfaceView.emulatorPtr
                                )
                            }
                            if (success) {
                                isPaused = !isPaused
                            }
                        }
                    }) {
                    Icon(
                        imageVector = if (isPaused) Icons.Default.PlayArrow
                        else Icons.Default.Done,
                        contentDescription = if (isPaused) "Resume" else "Pause"
                    )
                }
            }

            // Menu button
            IconButton(onClick = { showMenu = true }) {
                Icon(Icons.Default.Menu, contentDescription = "Menu")
            }

            // Dropdown menu
            DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                DropdownMenuItem(text = { Text("Open ROM") }, onClick = {
                    showMenu = false
                    // Launch file picker for any file type (Android doesn't support
                    // .gb/.gbc MIME types well)
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

                    HorizontalDivider()

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
                .weight(1f)
                .background(Color.Black),
            contentAlignment = Alignment.Center
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
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(bottom = 36.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            // D-Pad
            GameBoyDPad(
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.padding(16.dp)
            )

            // Action Buttons
            GameBoyActionButtons(
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.padding(16.dp)
            )
        }

        // Start/Select buttons positioned in center bottom
        Row(
            horizontalArrangement = Arrangement.spacedBy(16.dp),
            modifier = Modifier
                .align(Alignment.BottomCenter)
                .padding(bottom = 8.dp)
        ) {
            GameBoyStartSelectButton(
                text = "SELECT",
                buttonId = RustBridge.BUTTON_SELECT,
                onPress = onButtonPress,
                onRelease = onButtonRelease
            )
            GameBoyStartSelectButton(
                text = "START",
                buttonId = RustBridge.BUTTON_START,
                onPress = onButtonPress,
                onRelease = onButtonRelease
            )
        }
    }
}

@Composable
fun GameBoyDPad(
    onButtonPress: (Int) -> Unit, onButtonRelease: (Int) -> Unit, modifier: Modifier = Modifier
) {
    val pressedStates = remember {
        mutableStateMapOf(
            RustBridge.BUTTON_UP to false,
            RustBridge.BUTTON_DOWN to false,
            RustBridge.BUTTON_LEFT to false,
            RustBridge.BUTTON_RIGHT to false
        )
    }

    Box(modifier = modifier.size(120.dp), contentAlignment = Alignment.Center) {
        // D-Pad background cross shape
        Canvas(modifier = Modifier.fillMaxSize()) {
            val centerX = size.width / 2
            val centerY = size.height / 2
            val thickness = 36.dp.toPx()
            val length = 100.dp.toPx()
            val cornerRadius = 6.dp.toPx()

            val dpadPath = Path().apply {
                // Horizontal bar
                addRoundRect(
                    androidx.compose.ui.geometry.RoundRect(
                        left = centerX - length / 2,
                        top = centerY - thickness / 2,
                        right = centerX + length / 2,
                        bottom = centerY + thickness / 2,
                        cornerRadius = CornerRadius(cornerRadius)
                    )
                )
                // Vertical bar
                addRoundRect(
                    androidx.compose.ui.geometry.RoundRect(
                        left = centerX - thickness / 2,
                        top = centerY - length / 2,
                        right = centerX + thickness / 2,
                        bottom = centerY + length / 2,
                        cornerRadius = CornerRadius(cornerRadius)
                    )
                )
            }

            // Draw main D-Pad
            drawPath(path = dpadPath, color = DPadColor)

            // Pressed state overlay
            val buttonRadius = 16.dp.toPx()
            val buttonOffset = 34.dp.toPx()

            if (pressedStates[RustBridge.BUTTON_UP] == true) {
                drawCircle(
                    DPadColor.copy(alpha = 0.8f),
                    buttonRadius,
                    center = Offset(centerX, centerY - buttonOffset)
                )
            }
            if (pressedStates[RustBridge.BUTTON_DOWN] == true) {
                drawCircle(
                    DPadColor.copy(alpha = 0.8f),
                    buttonRadius,
                    center = Offset(centerX, centerY + buttonOffset)
                )
            }
            if (pressedStates[RustBridge.BUTTON_LEFT] == true) {
                drawCircle(
                    DPadColor.copy(alpha = 0.8f),
                    buttonRadius,
                    center = Offset(centerX - buttonOffset, centerY)
                )
            }
            if (pressedStates[RustBridge.BUTTON_RIGHT] == true) {
                drawCircle(
                    DPadColor.copy(alpha = 0.8f),
                    buttonRadius,
                    center = Offset(centerX + buttonOffset, centerY)
                )
            }
        }

        // Invisible touch areas for each direction
        Column(
            horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxSize()
        ) {
            // UP
            DPadButton(
                buttonId = RustBridge.BUTTON_UP,
                pressedStates = pressedStates,
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.size(36.dp, 32.dp)
            )

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxWidth()
            ) {
                // LEFT
                DPadButton(
                    buttonId = RustBridge.BUTTON_LEFT,
                    pressedStates = pressedStates,
                    onButtonPress = onButtonPress,
                    onButtonRelease = onButtonRelease,
                    modifier = Modifier.size(32.dp, 36.dp)
                )

                Spacer(modifier = Modifier.width(36.dp))

                // RIGHT
                DPadButton(
                    buttonId = RustBridge.BUTTON_RIGHT,
                    pressedStates = pressedStates,
                    onButtonPress = onButtonPress,
                    onButtonRelease = onButtonRelease,
                    modifier = Modifier.size(32.dp, 36.dp)
                )
            }

            // DOWN
            DPadButton(
                buttonId = RustBridge.BUTTON_DOWN,
                pressedStates = pressedStates,
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.size(36.dp, 32.dp)
            )
        }
    }
}

@Composable
private fun DPadButton(
    buttonId: Int,
    pressedStates: MutableMap<Int, Boolean>,
    onButtonPress: (Int) -> Unit,
    onButtonRelease: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    val interactionSource = remember { MutableInteractionSource() }

    LaunchedEffect(interactionSource) {
        interactionSource.interactions.collect { interaction ->
            when (interaction) {
                is PressInteraction.Press -> {
                    pressedStates[buttonId] = true
                    onButtonPress(buttonId)
                }

                is PressInteraction.Release -> {
                    pressedStates[buttonId] = false
                    onButtonRelease(buttonId)
                }

                is PressInteraction.Cancel -> {
                    pressedStates[buttonId] = false
                    onButtonRelease(buttonId)
                }
            }
        }
    }

    Box(modifier = modifier.clickable(interactionSource = interactionSource, indication = null) {})
}

@Composable
fun GameBoyActionButtons(
    onButtonPress: (Int) -> Unit, onButtonRelease: (Int) -> Unit, modifier: Modifier = Modifier
) {
    Column(
        modifier = modifier,
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Row(
            horizontalArrangement = Arrangement.spacedBy(24.dp),
            verticalAlignment = Alignment.CenterVertically
        ) {
            // B Button (left)
            GameBoyCircularButton(
                text = "B",
                buttonId = RustBridge.BUTTON_B,
                onPress = onButtonPress,
                onRelease = onButtonRelease,
                color = ActionButtonColor
            )

            // A Button (right)
            GameBoyCircularButton(
                text = "A",
                buttonId = RustBridge.BUTTON_A,
                onPress = onButtonPress,
                onRelease = onButtonRelease,
                color = ActionButtonColor
            )
        }
    }
}

@Composable
fun GameBoyCircularButton(
    text: String,
    buttonId: Int,
    onPress: (Int) -> Unit,
    onRelease: (Int) -> Unit,
    color: Color,
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

    Box(
        modifier = modifier
            .size(56.dp)
            .clickable(
                interactionSource = interactionSource, indication = null
            ) {}, contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            drawCircle(isPressed = isPressed, color = color, radius = size.minDimension / 2)
        }

        Text(text = text, color = Color.White, fontSize = 18.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
fun GameBoyStartSelectButton(
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

    Box(
        modifier = modifier
            .size(width = 48.dp, height = 20.dp)
            .clickable(
                interactionSource = interactionSource, indication = null
            ) {}, contentAlignment = Alignment.Center
    ) {
        // Draw rounded rectangular button
        Canvas(modifier = Modifier.fillMaxSize()) {
            drawRoundRect(
                isPressed = isPressed,
                color = StartSelectButtonColor,
                size = size,
                cornerRadius = CornerRadius(8.dp.toPx())
            )
        }

        Text(
            text = text,
            color = Color.White,
            fontSize = 9.sp,
            fontWeight = FontWeight.Bold,
            modifier = Modifier.offset(y = (-4).dp)
        )
    }
}

private fun DrawScope.drawCircle(isPressed: Boolean, color: Color, radius: Float) {
    drawCircle(
        color = if (isPressed) color.copy(alpha = 0.8f) else color, radius = radius, center = center
    )
}

private fun DrawScope.drawRoundRect(
    isPressed: Boolean,
    color: Color,
    size: Size,
    cornerRadius: CornerRadius,
) {
    drawRoundRect(
        color = if (isPressed) color.copy(alpha = 0.8f) else color,
        topLeft = Offset.Zero,
        size = size,
        cornerRadius = cornerRadius
    )
}
