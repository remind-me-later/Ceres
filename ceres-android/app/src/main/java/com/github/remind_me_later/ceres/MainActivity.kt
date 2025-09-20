package com.github.remind_me_later.ceres

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.BackHandler
import androidx.activity.compose.LocalActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.detectDragGestures
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Menu
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.core.content.edit
import androidx.core.net.toUri
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.documentfile.provider.DocumentFile
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.viewmodel.compose.viewModel
import com.github.remind_me_later.ceres.ui.theme.CeresTheme
import kotlinx.coroutines.flow.collectLatest
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream

private val DPadColor = Color(0x4DFFFFFF)
private val ActionButtonColor = Color(0x4DFFFFFF)
private val StartSelectButtonColor = Color(0x4DFFFFFF)

// To allow MainActivity to access the current EmulatorSurfaceView
// This is a bit of a workaround for the importSaveFile functionality.
// Ideally, this interaction would also go through the ViewModel or a shared service.
private var currentEmulatorSurfaceView: EmulatorSurfaceView? = null

class MainActivity : ComponentActivity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            CeresTheme {
                Surface(
                    modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background
                ) {
                    CeresApp()
                }
            }
        }
    }

    fun loadRomFromUri(surfaceView: EmulatorSurfaceView?, uri: Uri) {
        surfaceView ?: return
        try {
            Log.d("MainActivity", "Loading ROM from URI: $uri")

            val tempFile = copyUriToTempFile(uri)
            if (tempFile != null) {
                val originalFileName = getFileNameFromUri(uri) ?: "unknown_rom.gb"
                val savPath = getSavPathForRom(originalFileName)
                surfaceView.setSavPath(savPath)

                val existingSavPath = if (File(savPath).exists()) savPath else null

                val success = RustBridge.loadRom(
                    surfaceView.emulatorPtr, tempFile.absolutePath, existingSavPath
                )
                if (success) {
                    Log.d("MainActivity", "ROM loaded successfully: ${tempFile.absolutePath}")
                    Log.d("MainActivity", "SAV path: $savPath")
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

    private fun copyUriToTempFile(uri: Uri): File? {
        return try {
            val inputStream: InputStream? = contentResolver.openInputStream(uri)
            if (inputStream == null) {
                Log.e("MainActivity", "Could not open input stream for URI: $uri")
                return null
            }

            val fileName = getFileNameFromUri(uri) ?: "temp_rom.gb"
            val sanitizedFileName = fileName.replace(Regex("[^a-zA-Z0-9._-]"), "_")
            val tempFile = File(cacheDir, "roms/$sanitizedFileName")
            tempFile.parentFile?.mkdirs()

            if (tempFile.exists()) {
                tempFile.delete()
            }
            Log.d("MainActivity", "Copying ROM to: ${tempFile.absolutePath}")

            val outputStream = FileOutputStream(tempFile)
            inputStream.use { input ->
                outputStream.use { output ->
                    val buffer = ByteArray(8192)
                    var bytesRead: Int
                    var totalBytes = 0L
                    while (input.read(buffer).also { bytesRead = it } != -1) {
                        output.write(buffer, 0, bytesRead)
                        totalBytes += bytesRead
                    }
                    Log.d("MainActivity", "ROM copied successfully, size: $totalBytes bytes")
                }
            }
            if (tempFile.exists() && tempFile.length() > 0) tempFile else {
                Log.e("MainActivity", "Copied file is empty or does not exist")
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
            uri.lastPathSegment
        } catch (e: Exception) {
            Log.e("MainActivity", "Error getting file name: ${e.message}")
            uri.lastPathSegment
        }
    }

    private fun getSavPathForRom(romFileName: String): String {
        val savFileName = romFileName.substringBeforeLast(".") + ".sav"
        return File(filesDir, savFileName).absolutePath
    }

    override fun onDestroy() {
        cleanupTempRomFiles()
        currentEmulatorSurfaceView = null // Clear the reference
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

    fun importSaveFile(uri: Uri) {
        currentEmulatorSurfaceView?.let { surfaceView ->
            try {
                val currentSavPath = surfaceView.getCurrentSavPath()
                if (currentSavPath != null) {
                    contentResolver.openInputStream(uri)?.use { inputStream ->
                        File(currentSavPath).outputStream().use { outputStream ->
                            inputStream.copyTo(outputStream)
                        }
                    }
                    Log.d("MainActivity", "Save file imported successfully to: $currentSavPath")
                } else {
                    Log.w("MainActivity", "No ROM loaded, cannot import save file")
                }
            } catch (e: Exception) {
                Log.e("MainActivity", "Error importing save file: ${e.message}", e)
            }
        }
    }

    override fun onPause() {
        super.onPause()
        // ViewModel is not directly commanding saveRAM on activity pause anymore.
        // This logic will be handled by EmulatorScreen observing lifecycle or commands.
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

    // This effect is not strictly necessary anymore as pause/resume are handled
    // by commands or directly in EmulatorScreen's lifecycle.
    // LaunchedEffect(currentScreen) {
    // if (currentScreen == Screen.Emulator) {
    // emulatorViewModel.resume() // Consider if this is needed or handled by surface creation
    // } else {
    // emulatorViewModel.pause()
    // }
    // }

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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun RomListScreen(
    emulatorViewModel: EmulatorViewModel,
    onRomSelected: (Uri) -> Unit,
    isGameRunning: Boolean,
    onReturnToGame: () -> Unit
) {
    val context = LocalContext.current
    var romFolderUri by remember { mutableStateOf(getRomFolderUri(context)) }
    var showMenu by remember { mutableStateOf(false) }

    val directoryPickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocumentTree(), onResult = { uri ->
            uri?.let {
                context.contentResolver.takePersistableUriPermission(
                    it, Intent.FLAG_GRANT_READ_URI_PERMISSION
                )
                saveRomFolderUri(context, it)
                romFolderUri = it
            }
        })

    val saveFilePickerLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument(), onResult = { uri ->
            uri?.let {
                if (context is MainActivity) {
                    context.importSaveFile(it)
                }
            }
        })

    Scaffold(
        topBar = {
            TopAppBar(title = { Text("Roms") }, actions = {
                IconButton(onClick = { showMenu = true }) {
                    Icon(Icons.Default.Menu, contentDescription = "Menu")
                }
                DropdownMenu(expanded = showMenu, onDismissRequest = { showMenu = false }) {
                    DropdownMenuItem(text = { Text("Select ROM Folder") }, onClick = {
                        showMenu = false
                        directoryPickerLauncher.launch(null)
                    })
                    DropdownMenuItem(text = { Text("Import Save File") }, onClick = {
                        showMenu = false
                        if (currentEmulatorSurfaceView?.getCurrentSavPath() != null) {
                            saveFilePickerLauncher.launch(arrayOf("*/*"))
                        } else {
                            // Optionally show a toast or message that no game is loaded
                            Log.w(
                                "RomListScreen",
                                "Cannot import save, no game loaded or sav path not set."
                            )
                        }
                    })
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text("Speed 1x") },
                        onClick = { emulatorViewModel.setSpeed(1); showMenu = false })
                    DropdownMenuItem(
                        text = { Text("Speed 2x") },
                        onClick = { emulatorViewModel.setSpeed(2); showMenu = false })
                    DropdownMenuItem(
                        text = { Text("Speed 4x") },
                        onClick = { emulatorViewModel.setSpeed(4); showMenu = false })
                    HorizontalDivider()
                    DropdownMenuItem(
                        text = { Text("Toggle Mute") },
                        onClick = { emulatorViewModel.toggleMute(); showMenu = false })
                }
            })
        }) { paddingValues ->
        Column(modifier = Modifier.padding(paddingValues)) {
            if (romFolderUri == null) {
                Box(
                    modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center
                ) {
                    Button(onClick = { directoryPickerLauncher.launch(null) }) {
                        Text("Select ROMs Folder")
                    }
                }
            } else {
                val romFiles = remember(romFolderUri) {
                    val tree = DocumentFile.fromTreeUri(context, romFolderUri!!)
                    tree?.listFiles()?.filter {
                        it.isFile && (it.name?.endsWith(".gb") == true || it.name?.endsWith(
                            ".gbc"
                        ) == true)
                    } ?: emptyList()
                }
                LazyColumn(modifier = Modifier.fillMaxSize()) {
                    if (isGameRunning) {
                        item {
                            ListItem(
                                headlineContent = { Text("Return to Game") },
                                leadingContent = {
                                    Icon(
                                        Icons.Default.PlayArrow, contentDescription = "Play"
                                    )
                                },
                                modifier = Modifier.clickable { onReturnToGame() })
                            HorizontalDivider()
                        }
                    }
                    items(romFiles) { file ->
                        ListItem(
                            headlineContent = { Text(file.name ?: "") },
                            modifier = Modifier.clickable { onRomSelected(file.uri) })
                    }
                }
            }
        }
    }
}

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
            currentEmulatorSurfaceView = it // Set the global reference
        }
    }

    DisposableEffect(emulatorSurfaceView, lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            when (event) {
                Lifecycle.Event.ON_RESUME -> emulatorSurfaceView.resume() // Resume rendering when screen is visible
                Lifecycle.Event.ON_PAUSE -> emulatorSurfaceView.pause()   // Pause rendering when screen is not visible
                Lifecycle.Event.ON_DESTROY -> {
                    emulatorSurfaceView.cleanup() // Clean up when the composable is destroyed
                    if (currentEmulatorSurfaceView == emulatorSurfaceView) {
                        currentEmulatorSurfaceView = null // Clear the global reference
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
            if (currentEmulatorSurfaceView == emulatorSurfaceView) {
                currentEmulatorSurfaceView = null
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

    LaunchedEffect(romUri, isNewSelection, emulatorSurfaceView) {
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
            VirtualAnalogStick(
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.padding(16.dp)
            )
            GameBoyActionButtons(
                onButtonPress = onButtonPress,
                onButtonRelease = onButtonRelease,
                modifier = Modifier.padding(16.dp)
            )
        }
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
fun VirtualAnalogStick(
    onButtonPress: (Int) -> Unit, onButtonRelease: (Int) -> Unit, modifier: Modifier = Modifier
) {
    var thumbPosition by remember { mutableStateOf(Offset.Zero) }
    val radius = 50.dp
    val thumbRadius = 30.dp
    var pressedButtons by remember { mutableStateOf(emptySet<Int>()) }

    Box(
        modifier = modifier
            .size(radius * 2)
            .pointerInput(Unit) {
                detectDragGestures(
                    onDragEnd = {
                        thumbPosition = Offset.Zero
                        pressedButtons.forEach { onButtonRelease(it) }
                        pressedButtons = emptySet()
                    }) { change, dragAmount ->
                    change.consume()
                    val newPosition = thumbPosition + dragAmount
                    val distance = newPosition.getDistance()
                    thumbPosition = if (distance > radius.toPx()) {
                        newPosition.div(distance) * radius.toPx()
                    } else {
                        newPosition
                    }

                    val newPressedButtons = mutableSetOf<Int>()
                    if (thumbPosition != Offset.Zero) {
                        val angle = Math.toDegrees(
                            kotlin.math.atan2(thumbPosition.y, thumbPosition.x).toDouble()
                        )
                        when {
                            angle > -45 && angle <= 45 -> newPressedButtons.add(RustBridge.BUTTON_RIGHT)
                            angle > 45 && angle <= 135 -> newPressedButtons.add(RustBridge.BUTTON_DOWN)
                            angle > 135 || angle <= -135 -> newPressedButtons.add(RustBridge.BUTTON_LEFT)
                            angle > -135 -> newPressedButtons.add(RustBridge.BUTTON_UP)
                        }
                    }

                    val released = pressedButtons - newPressedButtons
                    val pressed = newPressedButtons - pressedButtons

                    released.forEach { onButtonRelease(it) }
                    pressed.forEach { onButtonPress(it) }

                    pressedButtons = newPressedButtons
                }
            }, contentAlignment = Alignment.Center
    ) {
        Canvas(modifier = Modifier.fillMaxSize()) {
            drawCircle(
                color = DPadColor, radius = radius.toPx()
            )
            drawCircle(
                color = Color.White, radius = thumbRadius.toPx(), center = center + thumbPosition
            )
        }
    }
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
            GameBoyCircularButton(
                text = "B",
                buttonId = RustBridge.BUTTON_B,
                onPress = onButtonPress,
                onRelease = onButtonRelease
            )
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
        Canvas(modifier = Modifier.fillMaxSize()) {
            val radius = size.minDimension / 2
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
        Canvas(modifier = Modifier.fillMaxSize()) {
            val cornerRadius = 8.dp.toPx()
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
