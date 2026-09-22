# chip8-rs

A CHIP-8 emulator written in Rust, using [minifb](https://crates.io/crates/minifb) for the window and input.

## Features

- Full CHIP-8 instruction set
- A 440 Hz beep plays while the sound timer is running (silent if no audio device is found)
- 700 Hz CPU and 60 Hz timers, independent of the display refresh rate
- 64x32 display, scaled 10x
- `FX0A` waits for a newly pressed key, not one that was already held
- Stack overflow and underflow are reported as errors instead of panicking
- Unit tests for the core opcodes

## Usage

```bash
cargo run --release -- path/to/rom.ch8
```

Press `Esc` to quit.

## Controls

The original CHIP-8 hex keypad is mapped onto the left side of a QWERTY keyboard:

```
CHIP-8        Keyboard
1 2 3 C       1 2 3 4
4 5 6 D       Q W E R
7 8 9 E       A S D F
A 0 B F       Z X C V
```

## Tests

```bash
cargo test
```

## Known limitations

- Quirks follow modern conventions: `8XY6`/`8XYE` shift `Vx` in place, and `FX55`/`FX65` leave `I` unchanged.
- A ROM that reads, writes or fetches outside the 4 KB of memory halts the emulator with an error instead of wrapping.

## Test ROMs

The [Timendus chip8-test-suite](https://github.com/Timendus/chip8-test-suite) is a good way to check correctness.
