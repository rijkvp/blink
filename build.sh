#!/bin/sh
name=blink
target=x86_64-unknown-linux-gnu
path=./target/$target/release/$name

# Use nightly custom build settings
cargo +nightly build -Z build-std=std,panic_abort -Z build-std-features=panic_immediate_abort --target $target --release
build_size=$(numfmt --grouping $(ls -la $path | awk '{print $5}'))

# Compress binary using upx
upx -qq --best --lzma $path
compress_size=$(numfmt --grouping $(ls -la $path | awk '{print $5}'))

# Install
cp $path ~/.local/bin/$name

echo "Build:        $build_size bytes"
echo "Compressed:   $compress_size bytes"