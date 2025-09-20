package com.github.remind_me_later.ceres.util

import android.content.Context
import android.net.Uri
import androidx.core.content.edit
import androidx.core.net.toUri
import com.github.remind_me_later.ceres.RustBridge

private const val PREFS_NAME = "CeresPrefs"
private const val KEY_ROM_FOLDER_URI = "romFolderUri"
private const val KEY_COLOR_CORRECTION_MODE = "colorCorrectionMode"
private const val KEY_SHADER_OPTION = "shaderOption"

fun saveRomFolderUri(context: Context, uri: Uri) {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    prefs.edit { putString(KEY_ROM_FOLDER_URI, uri.toString()) }
}

fun getRomFolderUri(context: Context): Uri? {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    val uriString = prefs.getString(KEY_ROM_FOLDER_URI, null)
    return uriString?.toUri()
}

fun saveColorCorrectionMode(context: Context, mode: Int) {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    prefs.edit { putInt(KEY_COLOR_CORRECTION_MODE, mode) }
}

fun getColorCorrectionMode(context: Context): Int {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    return prefs.getInt(KEY_COLOR_CORRECTION_MODE, RustBridge.COLOR_MODERN_BALANCED)
}

fun saveShaderOption(context: Context, shader: Int) {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    prefs.edit { putInt(KEY_SHADER_OPTION, shader) }
}

fun getShaderOption(context: Context): Int {
    val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
    return prefs.getInt(KEY_SHADER_OPTION, RustBridge.SHADER_LCD)
}
