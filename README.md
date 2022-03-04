# Blink

[![CI](https://github.com/rijkvp/blink/actions/workflows/ci.yml/badge.svg)](https://github.com/rijkvp/blink/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/blink-timer)](https://crates.io/crates/blink-timer)

A <1 MB minimal break notifier program. Helps you remeber to blink during long computer usage.

## Installation

You can download the latest executable from [GitHub releases](https://github.com/rijkvp/blink/releases).

Or you can compile `blink-timer` from [Crates.io](https://crates.io/crates/blink-timer): `cargo install blink-timer`

## Usage

Run `blink` to start the timer. A default `blink.toml` config will be generated in your system config directory (`~/.config` on Linux) if not already present. You can specify a different config file using `blink -c [config file path]`.

