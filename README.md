# Blink

A <100 KB minimal break notifier program for Linux. 
Helps you remeber to blink your eyes during long computer usage.

## Usage

`blink [break interval] [sound path] [activity timeout]`

- **Break interval:** Interval between breaks in seconds. Defaults to 25m / 1500s.
- **Sound path:** Absolute path to a sound file to play on each break. Only supports OGG files. Defaults to none.
- **Activity timeout:** The time the keyboard and mouse must be inactive for the timer to stop running. Defaults to 30s.


## Building

I used some tricks from [min-sized-rust](https://github.com/johnthagen/min-sized-rust) to optimize the binary size.
Use the `build.sh` script to build the program with optimized size results.
