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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
                        if ((context as? MainActivity)?.currentEmulatorSurfaceView?.getCurrentSavPath() != null) {
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
