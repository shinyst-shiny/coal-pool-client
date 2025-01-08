#!/bin/bash

# Set the application name
APP_NAME="excalivator-client"

# Output directory for built binaries
OUTPUT_DIR="dist"
BUILD_DIR="target"

# List of target platforms
PLATFORMS=(
    "x86_64-apple-darwin"
    "aarch64-apple-darwin"
    "aarch64-unknown-linux-musl"
    "x86_64-unknown-linux-musl"
    "x86_64-pc-windows-gnu"
)

# Ensure required tools are installed
check_tools() {
    echo "Checking required tools..."
    for tool in cargo cargo-zigbuild tar zip; do
        if ! command -v $tool &>/dev/null; then
            echo "Error: $tool is not installed. Please install it first."
            exit 1
        fi
    done
}

# Build for a specific platform
build_for_target() {
    local target=$1
    echo "Building for $target..."
    RUSTFLAGS="--remap-path-prefix=$(pwd)=." cross build --release --target "$target"

    # Handle Windows executable naming
    local binary_name=$APP_NAME
    [[ "$target" == *"windows"* ]] && binary_name="$APP_NAME.exe"

    # Strip debug symbols
    case "$target" in
        *"darwin"*) strip -x "$BUILD_DIR/$target/release/$binary_name" ;;
        *"linux"*) strip "$BUILD_DIR/$target/release/$binary_name" ;;
        *"windows"*) x86_64-w64-mingw32-strip "$BUILD_DIR/$target/release/$binary_name" ;;
    esac

    # Copy to output directory
    mkdir -p "$OUTPUT_DIR/$target"
    cp "$BUILD_DIR/$target/release/$binary_name" "$OUTPUT_DIR/$target/"
}

# Package binaries into archives
package_binaries() {
    for target in "${PLATFORMS[@]}"; do
        echo "Packaging $target..."
        local package_name="${APP_NAME}-${target}"

        # Use zip for Windows, tar.gz for others
        if [[ "$target" == *"windows"* ]]; then
            zip -j "$OUTPUT_DIR/$package_name.zip" "$OUTPUT_DIR/$target/"*
        else
            tar -czf "$OUTPUT_DIR/$package_name.tar.gz" -C "$OUTPUT_DIR/$target" .
        fi
    done
}

# Main script execution
main() {
    check_tools

    # Clean up old builds
    echo "Cleaning up old builds..."
    rm -rf "$BUILD_DIR"
    rm -rf "$OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"

    # Build for all platforms
    for target in "${PLATFORMS[@]}"; do
        build_for_target "$target"
    done

    # Package binaries
    package_binaries

    echo "Build and packaging completed. Binaries are in the $OUTPUT_DIR directory."
}

# Run the main function
main
