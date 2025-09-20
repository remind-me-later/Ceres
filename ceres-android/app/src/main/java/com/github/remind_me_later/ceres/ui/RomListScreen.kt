package com.github.remind_me_later.ceres.ui

import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.outlined.Home
import androidx.compose.material.icons.outlined.Settings
import androidx.compose.material3.Button
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
import androidx.documentfile.provider.DocumentFile
import com.github.remind_me_later.ceres.MainActivity
import com.github.remind_me_later.ceres.util.getRomFolderUri
import com.github.remind_me_later.ceres.util.saveRomFolderUri
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
SportsEsports
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