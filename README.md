# Blink

[![Release](https://github.com/rijkvp/blink/actions/workflows/release.yml/badge.svg)](https://github.com/rijkvp/blink/actions/workflows/release.yml)

Blink is a program that helps you to remember to take breaks (and blink your eyes) while using the computer. 

## Features

- ⏰ Configure multiple timers with different intervals.
- 🔔 Get a notification and play a sound when its time to take a break.
- ⌨️ Have timers automatically pause or reset when you are AFK.

## Installation

You can download the latest binaries from [GitHub releases](https://github.com/rijkvp/blink/releases).

## Usage

Run the `blinkd` daemon in the background.

You probably want the program to start automatically, on Linux, you can use the following systemd user service (place it in `/.config/systemd/user`):

```ini
[Install]
WantedBy=default.target

[Service]
Type=simple
# change this to your install location
ExecStart=%h/.local/bin/blinkd

[Unit]
Description=blinkd - break timer daemon
After=default.target
```

Place this file in `~/.config/systemd/user/blinkd.service` and enable it with `systemctl --user enable blinkd --now`.

The daemon can be controlled with the `blinkctl` program which has the following commands:

```
Usage: blinkctl <COMMAND>

Commands:
  status  Get status of current timers
  toggle  Toggle the timer
  reset   Reset all timers
  help    Print this message or the help of the given subcommand(s)
```

## Configuration

When no config file is found a default `blink.yaml` config file will be generated like the one below at `~/.config/blink/blink.yaml`. A different config file can optionally be specified using the `--config` flag.

```yaml
timers:
- interval: 20:00 # will notify every 20 minutes
  notification:
    title: Microbreak
    descriptions:
    - Look away from your screen for 20 seconds.
    - Roll your shoulders and stretch your neck.
    - Stand up and change your posture.
- interval: 01:00:00
  decline: 0.5 # first timer is 1 hour, then 30 minutes, 15 minutes, etc.
  notification:
    title: Take a break!
    descriptions:
    - You've been at your screen for {}. Time for a short walk or a stretch!
```

Optionally, you can play a sound (OGG file) or run a command when the timer is over. For example:

```yaml
timers:
- interval: 01:00:00
  sound: /path/to/mysound.ogg
  command: loginctl lock-session
  notification:
    title: Take a break!
```

## Input tracking

The optional `actived` daemon can be used on Linux to automatically reset the timers after a period of input inactivity, i.e. no keyboard or mouse input. The daemon must run as root user in order to access keyboard and mouse events. You can use the following systemd service:

```ini
[Unit]
Description=actived - activity daemon

[Service]
Type=simple
User=root
# change this to your install location
ExecStart=/usr/local/bin/actived

[Install]
WantedBy=multi-user.target
```

Then input tracking can be enabled by adding the following section to `blink.yaml`:

```yaml
input_tracking:
  pause_after: 00:30
  reset_after: 05:00
```
