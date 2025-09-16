package com.github.remind_me_later.ceres

import android.content.Context
import android.content.Intent
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
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
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
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.compose.ui.window.Dialog
import androidx.documentfile.provider.DocumentFile
import androidx.lifecycle.viewmodel.compose.viewModel
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream
import kotlinx.coroutines.launch
import androidx.core.content.edit
import androidx.core.net.toUri

private val DPadColor = Color(0x4DFFFFFF)
private val ActionButtonColor = Color(0x4DFFFFFF)
private val StartSelectButtonColor = Color(0x4DFFFFFF)

class MainActivity : ComponentActivity() {
    private var emulatorViewModel: EmulatorViewModel? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            MaterialTheme(colorScheme = darkColorScheme()) {
                Surface(
                    modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background
                ) { EmulatorScreen { vm -> emulatorViewModel = vm } }
            }
        }
    }

    fun loadRomFromUri(uri: Uri) {
        emulatorViewModel?.emulatorSurfaceView?.let { surfaceView ->
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

private const val PREFS_NAME = "CeresPrefs"
private const val KEY_ROM_FOLDER_URI = "romFolderUri"

private fun saveRomFolderUri(context: Context, uri: Uri) {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    prefs.edit { putString(KEY_ROM_FOLDER_URI, uri.toString()) }
}

private fun getRomFolderUri(context: Context): Uri? {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    val uriString = prefs.getString(KEY_ROM_FOLDER_URI, null)
    return uriString?.toUri()
}

@Composable
private fun RomListDialog(
    romFolderUri: Uri,
    onDismiss: () -> Unit,
    onRomSelected: (DocumentFile) -> Unit
) {
    val context = androidx.compose.ui.platform.LocalContext.current
    val romFiles = remember(romFolderUri) {
        val tree = DocumentFile.fromTreeUri(context, romFolderUri)
        tree?.listFiles()
            ?.filter { it.isFile && (it.name?.endsWith(".gb") == true || it.name?.endsWith(".gbc") == true) }
            ?: emptyList()
    }

    Dialog(onDismissRequest = onDismiss) {
        Surface(
            modifier = Modifier.fillMaxWidth(0.9f),
            shape = MaterialTheme.shapes.medium
        ) {
            Column {
                Text(
                    text = "Select a ROM",
                    style = MaterialTheme.typography.headlineSmall,
                    modifier = Modifier.padding(16.dp)
                )
                LazyColumn(modifier = Modifier.fillMaxWidth()) {
                    items(romFiles) { file ->
                        Text(
                            text = file.name ?: "",
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable { onRomSelected(file) }
                                .padding(16.dp)
                        )
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun EmulatorScreen(onViewModelCreated: (EmulatorViewModel) -> Unit) {
    val emulatorViewModel: EmulatorViewModel = viewModel()
    onViewModelCreated(emulatorViewModel)

    val context = androidx.compose.ui.platform.LocalContext.current
    var showMenu by remember { mutableStateOf(false) }
    var isPaused by remember { mutableStateOf(false) }
    var currentRomName by remember { mutableStateOf<String?>(null) }
    val scope = rememberCoroutineScope()

    var romFolderUri by remember { mutableStateOf(getRomFolderUri(context)) }
    var showRomListDialog by remember { mutableStateOf(false) }

    val directoryPickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree(),
        onResult = { uri ->
            uri?.let {
                context.contentResolver.takePersistableUriPermission(
                    it,
                    Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
                saveRomFolderUri(context, it)
                romFolderUri = it
                showRomListDialog = true
            }
        }
    )

    if (showRomListDialog && romFolderUri != null) {
        RomListDialog(
            romFolderUri = romFolderUri!!,
            onDismiss = { showRomListDialog = false },
            onRomSelected = {
                showRomListDialog = false
                if (context is MainActivity) {
                    context.loadRomFromUri(it.uri)
                    currentRomName = it.name
                }
            }
        )
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
            IconButton(
                onClick = {
                    scope.launch {
                        val success = if (isPaused) {
                            RustBridge.resumeEmulator(
                                emulatorViewModel.emulatorSurfaceView.emulatorPtr
                            )
                        } else {
                            RustBridge.pauseEmulator(
                                emulatorViewModel.emulatorSurfaceView.emulatorPtr
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

            // Menu button
            IconButton(onClick = { showMenu = true }) {
                Icon(Icons.Default.Menu, contentDescription = "Menu")
            }

            // Dropdown menu
            DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                DropdownMenuItem(text = { Text("Open ROM") }, onClick = {
                    showMenu = false
                    if (romFolderUri != null) {
                        showRomListDialog = true
                    } else {
                        directoryPickerLauncher.launch(null)
                    }
                })

                DropdownMenuItem(text = { Text("Select ROM Folder") }, onClick = {
                    showMenu = false
                    directoryPickerLauncher.launch(null)
                })

                HorizontalDivider()

                DropdownMenuItem(text = { Text("Speed 1x") }, onClick = {
                    showMenu = false
                    RustBridge.setSpeedMultiplier(emulatorViewModel.emulatorSurfaceView.emulatorPtr, 1)
                })
                DropdownMenuItem(text = { Text("Speed 2x") }, onClick = {
                    showMenu = false
                    RustBridge.setSpeedMultiplier(emulatorViewModel.emulatorSurfaceView.emulatorPtr, 2)
                })
                DropdownMenuItem(text = { Text("Speed 4x") }, onClick = {
                    showMenu = false
                    RustBridge.setSpeedMultiplier(emulatorViewModel.emulatorSurfaceView.emulatorPtr, 4)
                })

                HorizontalDivider()

                DropdownMenuItem(text = { Text("Toggle Mute") }, onClick = {
                    showMenu = false
                    RustBridge.toggleMute(emulatorViewModel.emulatorSurfaceView.emulatorPtr)
                })
            }
        })

        // Emulator Surface
        BoxWithConstraints(
            modifier = Modifier
                .fillMaxWidth()
                .weight(1f)
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
                factory = { emulatorViewModel.emulatorSurfaceView },
                modifier = modifier
            )
        }

        // Virtual Game Boy Controls
        GameBoyControls(
            onButtonPress = { buttonId ->
                RustBridge.pressButton(emulatorViewModel.emulatorSurfaceView.emulatorPtr, buttonId)
            }, onButtonRelease = { buttonId ->
                RustBridge.releaseButton(emulatorViewModel.emulatorSurfaceView.emulatorPtr, buttonId)
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
    var isUpPressed by remember { mutableStateOf(false) }
    var isDownPressed by remember { mutableStateOf(false) }
    var isLeftPressed by remember { mutableStateOf(false) }
    var isRightPressed by remember { mutableStateOf(false) }

    Box(modifier = modifier.size(120.dp), contentAlignment = Alignment.Center) {
        // D-Pad background cross shape
        Canvas(modifier = Modifier.fillMaxSize()) {
            val centerX = size.width / 2
            val centerY = size.height / 2
            val thickness = 36.dp.toPx()
            val length = 100.dp.toPx()
            val armLength = (length - thickness) / 2
            val cornerRadius = 6.dp.toPx()

            // Base D-pad shape
            // Horizontal bar
            drawRoundRect(
                color = DPadColor,
                topLeft = Offset(centerX - length / 2, centerY - thickness / 2),
                size = Size(length, thickness),
                cornerRadius = CornerRadius(cornerRadius)
            )

            // Vertical bar
            drawRoundRect(
                color = DPadColor,
                topLeft = Offset(centerX - thickness / 2, centerY - length / 2),
                size = Size(thickness, length),
                cornerRadius = CornerRadius(cornerRadius)
            )

            // Overlays for pressed states
            val pressOverlayColor = DPadColor.copy(alpha = 0.5f)

            if (isUpPressed) {
                drawRect(
                    color = pressOverlayColor,
                    topLeft = Offset(centerX - thickness / 2, centerY - length / 2),
                    size = Size(thickness, armLength)
                )
            }
            if (isDownPressed) {
                drawRect(
                    color = pressOverlayColor,
                    topLeft = Offset(centerX - thickness / 2, centerY + thickness / 2),
                    size = Size(thickness, armLength)
                )
            }
            if (isLeftPressed) {
                drawRect(
                    color = pressOverlayColor,
                    topLeft = Offset(centerX - length / 2, centerY - thickness / 2),
                    size = Size(armLength, thickness)
                )
            }
            if (isRightPressed) {
                drawRect(
                    color = pressOverlayColor,
                    topLeft = Offset(centerX + thickness / 2, centerY - thickness / 2),
                    size = Size(armLength, thickness)
                )
            }
        }

        // Invisible touch areas for each direction
        Column(
            horizontalAlignment = Alignment.CenterHorizontally, modifier = Modifier.fillMaxSize()
        ) {
            // UP
            InvisibleGameBoyButton(
                buttonId = RustBridge.BUTTON_UP,
                onPress = onButtonPress,
                onRelease = onButtonRelease,
                onStateChange = { isUpPressed = it },
                modifier = Modifier.size(36.dp, 42.dp) // Increased height for better touch area
            )

            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
                modifier = Modifier.fillMaxWidth()
            ) {
                // LEFT
                InvisibleGameBoyButton(
                    buttonId = RustBridge.BUTTON_LEFT,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease,
                    onStateChange = { isLeftPressed = it },
                    modifier = Modifier.size(42.dp, 36.dp) // Increased width
                )

                Spacer(modifier = Modifier.width(36.dp))

                // RIGHT
                InvisibleGameBoyButton(
                    buttonId = RustBridge.BUTTON_RIGHT,
                    onPress = onButtonPress,
                    onRelease = onButtonRelease,
                    onStateChange = { isRightPressed = it },
                    modifier = Modifier.size(42.dp, 36.dp) // Increased width
                )
            }

            // DOWN
            InvisibleGameBoyButton(
                buttonId = RustBridge.BUTTON_DOWN,
                onPress = onButtonPress,
                onRelease = onButtonRelease,
                onStateChange = { isDownPressed = it },
                modifier = Modifier.size(36.dp, 42.dp) // Increased height
            )
        }
    }
}

@Composable
fun InvisibleGameBoyButton(
    buttonId: Int,
    onPress: (Int) -> Unit,
    onRelease: (Int) -> Unit,
    onStateChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier
) {
    val interactionSource = remember { MutableInteractionSource() }

    LaunchedEffect(interactionSource) {
        interactionSource.interactions.collect { interaction ->
            when (interaction) {
                is PressInteraction.Press -> {
                    onPress(buttonId)
                    onStateChange(true)
                }

                is PressInteraction.Release -> {
                    onRelease(buttonId)
                    onStateChange(false)
                }

                is PressInteraction.Cancel -> {
                    onRelease(buttonId)
                    onStateChange(false)
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
                onRelease = onButtonRelease
            )

            // A Button (right)
            GameBoyCircularButton(
                text = "A",
                buttonId = RustBridge.BUTTON_A,
                onPress = onButtonPress,
                onRelease = onButtonRelease
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
        // Draw circular button
        Canvas(modifier = Modifier.fillMaxSize()) {
            val radius = size.minDimension / 2

            // Main button
            drawCircle(
                color = if (isPressed) ActionButtonColor.copy(alpha = 0.8f)
                else ActionButtonColor, radius = radius, center = center
            )
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
            val cornerRadius = 8.dp.toPx()

            // Main button
            drawRoundRect(
                color = if (isPressed) StartSelectButtonColor.copy(alpha = 0.8f)
                else StartSelectButtonColor, size = size, cornerRadius = CornerRadius(cornerRadius)
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
