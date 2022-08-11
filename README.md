# Blink

[![Release](https://github.com/rijkvp/blink/actions/workflows/release.yml/badge.svg)](https://github.com/rijkvp/blink/actions/workflows/release.yml)

Blink is a really small single-binary program that helps you to remember to take breaks (and blink your eyes) while using the computer. 

## Features

- Create multiple break types with different intervals, weights, timeouts, & decaying prompts.
- Notifications with customizable descriptions.
- Playing a sound when it's time to take a break.
- Keeping track if the computer is used: after a timeout the timer will be paused or reset.

## Installation

You can download the latest executable from [GitHub releases](https://github.com/rijkvp/blink/releases).


## Usage

Start the executable in the background. You'll probably want to run it on system boot but you can try it out from a terminal.

## Configuration

A default `blink.toml` config file will be generated in your system config directory (`~/.config` on Linux) if not already present. 
You can specify a different config file using `blink -c [path]`.

Use the example below as a reference and to configure the behavior of the program to your needs. The duration type is specified as a string in seconds.

```toml
# The delay between each update.
update_delay = "1"
# How long it takes for the timer to pause after receiving no input.
input_timeout = "30"
# How long it takes for the timer to reset after receiving no input.
input_reset = "300"
# The delay between updates at which the timer resets. This is caused by your computer sleeping.
timeout_reset = "200"
# How long the notifications are displayed.
notification_timeout = "10"
# Resets the timer when the notification is pressed (only supported on Linux)
notification_press_reset = true
# Templates for the time description in the notification.
time_descriptions = ["Using the computer for {} minutes.", "Staring at the screen for {} minutes."]
# The path of the directory the sounds are loaded from. You can specify a different sound for earch break type.
sounds_dir = "/path/to/my/sounds"

# You can specify multiple break types / timers.
[[break]]
# Required: title of the break, will be shown on notifications.
title = "Micro break"
# Required: the interval between each break.
interval = "1200"
# A list of random descriptions. A random one will be shown in notifications
descriptions = ["Don't forget to blink your eyes.", "Look away from the screen for a moment.", "Make sure you have a good posture."]

[[break]]
title = "Computer break"
interval = "1800"
# How long it takes for the break to become 'active'.
timeout = "2000"
# Breaks with a higher weight will be chosen before others.
weight = 2
# The decay of the break interval after each prompt. A decay of 1.0 will multiply the interval with 0.5 after each prompt.
decay = 0.5
descriptions = ["Get away from behind your screen!", "Time to relax for a moment!"]
# The sound to play on each prompt. Refering to a file in the sounds_dir.
# Note that only the .ogg (vorbis) codec is supported!
# You can use ffmpeg to convert audio files: ffmpeg -i mysound.mp3 mysound.ogg
sound_file = "time-for-a-break.ogg"
# How long the sound should be played.
sound_duration = "10"
# A command that gets run on the break
command = 'mpv "https://www.youtube.com/watch?v=dQw4w9WgXcQ"'
```
