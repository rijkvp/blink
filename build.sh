#!/bin/sh
name=blink
target=x86_64-unknown-linux-gnu
path=./target/$target/release/$name
# Use nightly custom build settings
cargo +nightly build --verbose -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target $target --release
echo "Build size: $(ls -la $path | awk '{print $5}') bytes"
strip $path
echo "Stripped size: $(ls -la $path | awk '{print $5}') bytes"
# Cheat with UPX to optimize binary size
upx -q --best --lzma $path
echo "Optimized size: $(ls -la $path | awk '{print $5}') bytes"
# Install the binary
mkdir -p ./artifacts
cp $path ./artifacts/$name
