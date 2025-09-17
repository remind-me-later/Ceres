#!/bin/bash

# Build script for ceres-android
# This script builds the library for all Android architectures and copies them to the Android project

set -e

echo "Building ceres-android for Android targets..."

# Check if cargo-ndk is installed
if ! command -v cargo-ndk &> /dev/null; then
    echo "cargo-ndk is not installed. Install it with: cargo install cargo-ndk"
    exit 1
fi

BUILD_TYPE="release" # Change to "debug" for debug builds
if [ "$BUILD_TYPE" == "release" ]; then
    PROFILE="release"
else
    PROFILE="dev"
fi

# Android API level (match with app's minSdk)
API_LEVEL=29

# Android project path (relative to ceres-jni directory)
ANDROID_PROJECT_PATH="../ceres-android/app/src/main/jniLibs"

# Build for all Android architectures
declare -A TARGETS
TARGETS[arm64-v8a]="aarch64-linux-android"
TARGETS[armeabi-v7a]="armv7-linux-androideabi"
TARGETS[x86]="i686-linux-android"
TARGETS[x86_64]="x86_64-linux-android"

for android_arch in "${!TARGETS[@]}"; do
    rust_target="${TARGETS[$android_arch]}"
    echo "Building for $android_arch ($rust_target)..."
    
    # Build the library
    cargo ndk --target $android_arch --platform $API_LEVEL build --profile $PROFILE
    
    # Create the target directory in Android project if it doesn't exist
    mkdir -p "$ANDROID_PROJECT_PATH/$android_arch"
    
    # Copy the .so file to the Android project
    if [ -f "../target/$rust_target/$BUILD_TYPE/libceres_jni.so" ]; then
        cp "../target/$rust_target/$BUILD_TYPE/libceres_jni.so" "$ANDROID_PROJECT_PATH/$android_arch/"
        echo "✓ Copied libceres_jni.so to $ANDROID_PROJECT_PATH/$android_arch/"
    else
        echo "✗ Failed to find libceres_jni.so for $android_arch"
    fi
done

echo ""
echo "Build complete! Generated libraries have been copied to Android project:"
echo "  $ANDROID_PROJECT_PATH/"
echo ""
echo "Library locations:"
for android_arch in "${!TARGETS[@]}"; do
    if [ -f "$ANDROID_PROJECT_PATH/$android_arch/libceres_jni.so" ]; then
        echo "  ✓ $android_arch/libceres_jni.so"
    else
        echo "  ✗ $android_arch/libceres_jni.so (missing)"
    fi
done
