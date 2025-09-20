package com.github.remind_me_later.ceres

import android.net.Uri
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.ui.Modifier
import com.github.remind_me_later.ceres.ui.CeresApp
import com.github.remind_me_later.ceres.ui.theme.CeresTheme
import com.github.remind_me_later.ceres.util.cleanupTempRomFiles
import com.github.remind_me_later.ceres.util.importSaveFile
import com.github.remind_me_later.ceres.util.loadRomFromUri

class MainActivity : ComponentActivity() {
    var currentEmulatorSurfaceView: EmulatorSurfaceView? = null

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
        loadRomFromUri(this, surfaceView, uri)
    }

    fun importSaveFile(uri: Uri) {
        importSaveFile(this, uri, currentEmulatorSurfaceView)
    }

    override fun onDestroy() {
        cleanupTempRomFiles(this)
        currentEmulatorSurfaceView = null // Clear the reference
        super.onDestroy()
    }
}
