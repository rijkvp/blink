# Blink

[![Release](https://github.com/rijkvp/blink/actions/workflows/release.yml/badge.svg)](https://github.com/rijkvp/blink/actions/workflows/release.yml)

Blink is a program that helps you to remember to take breaks (and blink your eyes) while using the computer. 

## Features

- ⏰ Configure multiple timers with intervals, weights, timeouts & declining prompts.
- 🔔 Get a notification and play a sound when its time to take a break.
- ⌨️ Have timers automatically pause or reset when you are AFK.

## Installation

You can download the latest executable from [GitHub releases](https://github.com/rijkvp/blink/releases).

## Usage

Run the executable in the background. 
You probably want to automatically start the program when your PC boots.

## Configuration

When no config file is found a default `blink.yaml` config file will be generated like the one below.
A different config file can be specified using the `--config` flag.

```yaml
timers:
- interval: 20:00
  notification:
    title: Small break
    descriptions:
    - Time to get a cup of cofee.
    - Time to get away from your desk.
- interval: 60:00
  decline: 0.6
  notification:
    title: Big break
    descriptions:
    - Time to relax. You've been using the computer for {} minutes.
input_tracking:
  pause_after: 00:30
  reset_after: 02:00
```
