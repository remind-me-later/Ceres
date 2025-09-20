package com.github.remind_me_later.ceres.util

import android.content.Context
import android.net.Uri
import android.util.Log
import com.github.remind_me_later.ceres.EmulatorSurfaceView
import com.github.remind_me_later.ceres.RustBridge
import java.io.File
import java.io.FileOutputStream
import java.io.InputStream

fun loadRomFromUri(context: Context, surfaceView: EmulatorSurfaceView?, uri: Uri) {
    surfaceView ?: return
    try {
        Log.d("FileUtils", "Loading ROM from URI: $uri")

        val tempFile = copyUriToTempFile(context, uri)
        if (tempFile != null) {
            val originalFileName = getFileNameFromUri(context, uri) ?: "unknown_rom.gb"
            val savPath = getSavPathForRom(context, originalFileName)
            surfaceView.setSavPath(savPath)

            val existingSavPath = if (File(savPath).exists()) savPath else null

            val success = RustBridge.loadRom(
                surfaceView.emulatorPtr, tempFile.absolutePath, existingSavPath
            )
            if (success) {
                Log.d("FileUtils", "ROM loaded successfully: ${tempFile.absolutePath}")
                Log.d("FileUtils", "SAV path: $savPath")
            } else {
                Log.e("FileUtils", "Failed to load ROM: ${tempFile.absolutePath}")
            }
        } else {
            Log.e("FileUtils", "Failed to copy ROM file")
        }
    } catch (e: Exception) {
        Log.e("FileUtils", "Error loading ROM: ${e.message}")
    }
}

private fun copyUriToTempFile(context: Context, uri: Uri): File? {
    return try {
        val inputStream: InputStream? = context.contentResolver.openInputStream(uri)
        if (inputStream == null) {
            Log.e("FileUtils", "Could not open input stream for URI: $uri")
            return null
        }

        val fileName = getFileNameFromUri(context, uri) ?: "temp_rom.gb"
        val sanitizedFileName = fileName.replace(Regex("[^a-zA-Z0-9._-]"), "_")
        val tempFile = File(context.cacheDir, "roms/$sanitizedFileName")
        tempFile.parentFile?.mkdirs()

        if (tempFile.exists()) {
            tempFile.delete()
        }
        Log.d("FileUtils", "Copying ROM to: ${tempFile.absolutePath}")

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
                Log.d("FileUtils", "ROM copied successfully, size: $totalBytes bytes")
            }
        }
        if (tempFile.exists() && tempFile.length() > 0) tempFile else {
            Log.e("FileUtils", "Copied file is empty or does not exist")
            null
        }
    } catch (e: Exception) {
        Log.e("FileUtils", "Error copying ROM file: ${e.message}", e)
        null
    }
}

fun getFileNameFromUri(context: Context, uri: Uri): String? {
    return try {
        val cursor = context.contentResolver.query(uri, null, null, null, null)
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
        Log.e("FileUtils", "Error getting file name: ${e.message}")
        uri.lastPathSegment
    }
}

fun getSavPathForRom(context: Context, romFileName: String): String {
    val savFileName = romFileName.substringBeforeLast(".") + ".sav"
    return File(context.filesDir, savFileName).absolutePath
}

fun cleanupTempRomFiles(context: Context) {
    try {
        val romsDir = File(context.cacheDir, "roms")
        if (romsDir.exists()) {
            romsDir.listFiles()?.forEach { file ->
                if (file.isFile) {
                    file.delete()
                    Log.d("FileUtils", "Cleaned up temp ROM file: ${file.name}")
                }
            }
        }
    } catch (e: Exception) {
        Log.e("FileUtils", "Error cleaning up temp ROM files: ${e.message}")
    }
}

fun importSaveFile(context: Context, uri: Uri, surfaceView: EmulatorSurfaceView?) {
    surfaceView?.let {
        try {
            val currentSavPath = it.getCurrentSavPath()
            if (currentSavPath != null) {
                context.contentResolver.openInputStream(uri)?.use { inputStream ->
                    File(currentSavPath).outputStream().use { outputStream ->
                        inputStream.copyTo(outputStream)
                    }
                }
                Log.d("FileUtils", "Save file imported successfully to: $currentSavPath")
            } else {
                Log.w("FileUtils", "No ROM loaded, cannot import save file")
            }
        } catch (e: Exception) {
            Log.e("FileUtils", "Error importing save file: ${e.message}", e)
        }
    }
}
