mod audio;
mod cpu;

use std::time::{Duration, Instant};

use minifb::{Key, Window, WindowOptions};

use audio::Beeper;
use cpu::Cpu;

const WIDTH: usize = cpu::WIDTH;
const HEIGHT: usize = cpu::HEIGHT;

const SCALE: usize = 10;
const WIN_W: usize = WIDTH * SCALE;
const WIN_H: usize = HEIGHT * SCALE;

const CPU_HZ: f64 = 700.0;
const TIMER_HZ: f64 = 60.0;
const MAX_FRAME_TIME: f64 = 0.1;

const COLOR_ON: u32 = 0x00FF_FFFF;
const COLOR_OFF: u32 = 0x0000_0000;

fn render(display: &[u8], buffer: &mut [u32]) {
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let color = if display[y * WIDTH + x] != 0 {
                COLOR_ON
            } else {
                COLOR_OFF
            };
            for sy in 0..SCALE {
                for sx in 0..SCALE {
                    buffer[(y * SCALE + sy) * WIN_W + (x * SCALE + sx)] = color;
                }
            }
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rom>", args[0]);
        std::process::exit(1);
    }

    let mut cpu = Cpu::new();
    if let Err(e) = cpu.load_rom(&args[1]) {
        eprintln!("Failed to load ROM '{}': {}", args[1], e);
        std::process::exit(1);
    }

    let mut window = Window::new("CHIP-8", WIN_W, WIN_H, WindowOptions::default())
        .expect("Failed to create window");
    window.limit_update_rate(Some(Duration::from_micros(16_600)));

    let beeper = Beeper::new();
    let mut buffer = vec![0u32; WIN_W * WIN_H];

    let mut last = Instant::now();
    let mut cycle_accumulator = 0.0f64;
    let mut timer_accumulator = 0.0f64;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        cpu.set_keys(&window);

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f64().min(MAX_FRAME_TIME);
        last = now;

        cycle_accumulator += dt * CPU_HZ;
        while cycle_accumulator >= 1.0 {
            if let Err(e) = cpu.cycle() {
                eprintln!("CPU halted: {:?}", e);
                return;
            }
            cycle_accumulator -= 1.0;
        }

        timer_accumulator += dt * TIMER_HZ;
        while timer_accumulator >= 1.0 {
            cpu.update_timers();
            timer_accumulator -= 1.0;
        }
        beeper.set_active(cpu.sound_active());

        render(cpu.display(), &mut buffer);
        window
            .update_with_buffer(&buffer, WIN_W, WIN_H)
            .expect("Failed to update window");
    }
}
