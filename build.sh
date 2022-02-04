#!/bin/sh
path=target/x86_64-unknown-linux-gnu/release/blink
# Use nightly custom build settings
cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target x86_64-unknown-linux-gnu --release
echo "Build size:" $(ls -lah $path | awk '{print $5}')
strip $path
echo "Stripped size:" $(ls -lah $path | awk '{print $5}')
# Cheat with UPX to optimize binary size
upx --best --lzma $path
echo "Optimized size:" $(ls -lah $path | awk '{print $5}')
# Install the binary
cp $path ~/.local/bin/blink