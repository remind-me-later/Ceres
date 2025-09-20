package com.github.remind_me_later.ceres.ui

import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Button
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.ListItem
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.documentfile.provider.DocumentFile
import com.github.remind_me_later.ceres.MainActivity
import com.github.remind_me_later.ceres.RustBridge
import com.github.remind_me_later.ceres.util.getRomFolderUri
import com.github.remind_me_later.ceres.util.saveRomFolderUri
import com.github.remind_me_later.ceres.util.getColorCorrectionMode
import com.github.remind_me_later.ceres.util.saveColorCorrectionMode
import com.github.remind_me_later.ceres.util.getShaderOption
import com.github.remind_me_later.ceres.util.saveShaderOption
import com.github.remind_me_later.ceres.viewmodel.EmulatorViewModel

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun HomeScreen(
    emulatorViewModel: EmulatorViewModel,
    onRomSelected: (Uri) -> Unit,
    isGameRunning: Boolean,
    onReturnToGame: () -> Unit
) {
    val context = LocalContext.current
    var tabIndex by remember { mutableStateOf(0) }
    val tabIcons = listOf(
        Icons.Outlined.Home,
        Icons.Outlined.Settings
    )
    val tabTitles = listOf("Roms", "Settings")

    var romFolderUri by remember { mutableStateOf(getRomFolderUri(context)) }

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
        bottomBar = {
            NavigationBar {
                tabTitles.forEachIndexed { index, title ->
                    NavigationBarItem(
                        selected = tabIndex == index,
                        onClick = { tabIndex = index },
                        icon = { Icon(tabIcons[index], contentDescription = title) },
                        label = { Text(title) }
                    )
                }
            }
        }
    ) { paddingValues ->
        Box(modifier = Modifier.padding(paddingValues)) {
            when (tabIndex) {
                0 -> RomList(
                    romFolderUri = romFolderUri,
                    onSelectRomFolder = { directoryPickerLauncher.launch(null) },
                    onRomSelected = onRomSelected,
                    isGameRunning = isGameRunning,
                    onReturnToGame = onReturnToGame,
                    modifier = Modifier.fillMaxSize()
                )
                1 -> SettingsScreen(
                    emulatorViewModel = emulatorViewModel,
                    onSelectRomFolder = { directoryPickerLauncher.launch(null) },
                    onImportSaveFile = {
                        if ((context as? MainActivity)?.currentEmulatorSurfaceView?.getCurrentSavPath() != null) {
                            saveFilePickerLauncher.launch(arrayOf("*/*"))
                        } else {
                            Log.w(
                                "HomeScreen",
                                "Cannot import save, no game loaded or sav path not set."
                            )
                        }
                    },
                    modifier = Modifier.fillMaxSize()
                )
            }
        }
    }
}

@Composable
fun RomList(
    romFolderUri: Uri?,
    onSelectRomFolder: () -> Unit,
    onRomSelected: (Uri) -> Unit,
    isGameRunning: Boolean,
    onReturnToGame: () -> Unit,
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    Column(modifier = modifier) {
        if (romFolderUri == null) {
            Box(
                modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center
            ) {
                Button(onClick = { onSelectRomFolder() }) {
                    Text("Select ROMs Folder")
                }
            }
        } else {
            val romFiles = remember(romFolderUri) {
                val tree = DocumentFile.fromTreeUri(context, romFolderUri)
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
                            modifier = Modifier.clickable { onReturnToGame() }
                        )
                        HorizontalDivider()
                    }
                }
                items(romFiles) { file ->
                    ListItem(
                        headlineContent = { Text(file.name ?: "") },
                        modifier = Modifier.clickable { onRomSelected(file.uri) }
                    )
                }
            }
        }
    }
}

@Composable
fun SettingsScreen(
    emulatorViewModel: EmulatorViewModel,
    onSelectRomFolder: () -> Unit,
    onImportSaveFile: () -> Unit,
    modifier: Modifier = Modifier
) {
    val context = LocalContext.current
    var colorCorrectionMode by remember { mutableStateOf(getColorCorrectionMode(context)) }
    var shaderOption by remember { mutableStateOf(getShaderOption(context)) }
    var colorCorrectionExpanded by remember { mutableStateOf(false) }
    var shaderExpanded by remember { mutableStateOf(false) }

    val colorCorrectionOptions = mapOf(
        RustBridge.COLOR_CORRECT_CURVES to "Correct Curves",
        RustBridge.COLOR_DISABLED to "Disabled",
        RustBridge.COLOR_LOW_CONTRAST to "Low Contrast",
        RustBridge.COLOR_MODERN_BALANCED to "Modern Balanced",
        RustBridge.COLOR_MODERN_BOOST_CONTRAST to "Modern Boost Contrast",
        RustBridge.COLOR_REDUCE_CONTRAST to "Reduce Contrast"
    )

    val shaderOptions = mapOf(
        RustBridge.SHADER_CRT to "CRT",
        RustBridge.SHADER_LCD to "LCD",
        RustBridge.SHADER_NEAREST to "Nearest",
        RustBridge.SHADER_SCALE2X to "Scale2x",
        RustBridge.SHADER_SCALE3X to "Scale3x"
    )

    LazyColumn(modifier = modifier) {
        item {
            ListItem(
                headlineContent = { Text("Select ROM Folder") },
                modifier = Modifier.clickable { onSelectRomFolder() }
            )
        }
        item {
            ListItem(
                headlineContent = { Text("Import Save File") },
                modifier = Modifier.clickable { onImportSaveFile() }
            )
        }
        item { HorizontalDivider() }
        
        // Color Correction Mode Setting
        item {
            ListItem(
                headlineContent = { 
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Color Correction")
                        Spacer(modifier = Modifier.width(16.dp))
                        Box {
                            Button(
                                onClick = { colorCorrectionExpanded = true }
                            ) {
                                Text(colorCorrectionOptions[colorCorrectionMode] ?: "Unknown")
                            }
                            DropdownMenu(
                                expanded = colorCorrectionExpanded,
                                onDismissRequest = { colorCorrectionExpanded = false }
                            ) {
                                colorCorrectionOptions.forEach { (mode, name) ->
                                    DropdownMenuItem(
                                        text = { Text(name) },
                                        onClick = {
                                            colorCorrectionMode = mode
                                            saveColorCorrectionMode(context, mode)
                                            emulatorViewModel.setColorCorrectionMode(mode)
                                            colorCorrectionExpanded = false
                                        }
                                    )
                                }
                            }
                        }
                    }
                }
            )
        }

        // Shader Option Setting
        item {
            ListItem(
                headlineContent = {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Text("Shader")
                        Spacer(modifier = Modifier.width(16.dp))
                        Box {
                            Button(
                                onClick = { shaderExpanded = true }
                            ) {
                                Text(shaderOptions[shaderOption] ?: "Unknown")
                            }
                            DropdownMenu(
                                expanded = shaderExpanded,
                                onDismissRequest = { shaderExpanded = false }
                            ) {
                                shaderOptions.forEach { (shader, name) ->
                                    DropdownMenuItem(
                                        text = { Text(name) },
                                        onClick = {
                                            shaderOption = shader
                                            saveShaderOption(context, shader)
                                            emulatorViewModel.setShaderOption(shader)
                                            shaderExpanded = false
                                        }
                                    )
                                }
                            }
                        }
                    }
                }
            )
        }
        
        item { HorizontalDivider() }
        item {
            ListItem(
                headlineContent = { Text("Speed 1x") },
                modifier = Modifier.clickable { emulatorViewModel.setSpeed(1) }
            )
        }
        item {
            ListItem(
                headlineContent = { Text("Speed 2x") },
                modifier = Modifier.clickable { emulatorViewModel.setSpeed(2) }
            )
        }
        item {
            ListItem(
                headlineContent = { Text("Speed 4x") },
                modifier = Modifier.clickable { emulatorViewModel.setSpeed(4) }
            )
        }
        item { HorizontalDivider() }
        item {
            ListItem(
                headlineContent = { Text("Toggle Mute") },
                modifier = Modifier.clickable { emulatorViewModel.toggleMute() }
            )
        }
    }
}