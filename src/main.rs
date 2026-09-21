use std::fs::File;
use std::io::{Read, Result};
use std::time::{Duration, Instant};

use minifb::{Key, Window, WindowOptions};

const WIDTH: u8 = 64;
const HEIGHT: u8 = 32;
const MEMORY_SIZE: u16 = 4096;
const START_ADDR: &str = "0x200";
const FONT_ADDR: &str = "0x50";

const SCALE: usize = 10;
const WIN_W: usize = WIDTH as usize * SCALE;
const WIN_H: usize = HEIGHT as usize * SCALE;

const CPU_HZ: f64 = 700.0;
const TIMER_HZ: f64 = 60.0;
const MAX_FRAME_TIME: f64 = 0.1;

const COLOR_ON: u32 = 0x00FF_FFFF;
const COLOR_OFF: u32 = 0x0000_0000;

#[derive(Debug, PartialEq, Eq)]
enum CpuError {
    StackOverflow,
    StackUnderflow,
}

struct CPU {
    mem: [u8; 4096],
    v: [u8; 16],
    i: u16,
    pc: u16,
    stack: [u16; 16],
    sp: u8,
    delay_timer: u8,
    sound_timer: u8,
    display: [u8; (WIDTH as usize) * (HEIGHT as usize)],
    keys: [bool; 16],
    // When `Some((x, snapshot))`, the CPU is blocked on an `FX0A` instruction.
    // `x` is the target register and `snapshot` is the key state at the moment
    // `FX0A` executed, so only a *newly* pressed key is accepted.
    waiting_for_key: Option<(u8, [bool; 16])>,
}

impl CPU {
    pub fn new() -> Self {
        const FONTSET: [u8; 80] = [
            0xF0, 0x90, 0x90, 0x90, 0xF0,
            0x20, 0x60, 0x20, 0x20, 0x70,
            0xF0, 0x10, 0xF0, 0x80, 0xF0,
            0xF0, 0x10, 0xF0, 0x10, 0xF0,
            0x90, 0x90, 0xF0, 0x10, 0x10,
            0xF0, 0x80, 0xF0, 0x10, 0xF0,
            0xF0, 0x80, 0xF0, 0x90, 0xF0,
            0xF0, 0x10, 0x20, 0x40, 0x40,
            0xF0, 0x90, 0xF0, 0x90, 0xF0,
            0xF0, 0x90, 0xF0, 0x10, 0xF0,
            0xF0, 0x90, 0xF0, 0x90, 0x90,
            0xE0, 0x90, 0xE0, 0x90, 0xE0,
            0xF0, 0x80, 0x80, 0x80, 0xF0,
            0xE0, 0x90, 0x90, 0x90, 0xE0,
            0xF0, 0x80, 0xF0, 0x80, 0xF0,
            0xF0, 0x80, 0xF0, 0x80, 0x80 
        ];

        let mut mem = [0u8; 4096];

        let font_addr = u16::from_str_radix(FONT_ADDR.trim_start_matches("0x"), 16).unwrap();
        mem[font_addr as usize..(font_addr as usize + FONTSET.len())].copy_from_slice(&FONTSET);

        CPU {
            mem,
            v: [0; 16],
            i: 0,
            pc: u16::from_str_radix(START_ADDR.trim_start_matches("0x"), 16).unwrap(),
            stack: [0; 16],
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            display: [0; (WIDTH as usize) * (HEIGHT as usize)],
            keys: [false; 16],
            waiting_for_key: None,
        }
    }

     pub fn load_rom(&mut self, path: &str) -> Result<()> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let start = u16::from_str_radix(START_ADDR.trim_start_matches("0x"), 16).unwrap() as usize;
        let max_size = 0x1000 - start; 
        if buffer.len() > max_size {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "ROM is too large to fit in memory",
            ));
        }

        let end = start + buffer.len();
        self.mem[start..end].copy_from_slice(&buffer);

        Ok(())
    }

    fn fetch_opcode(&self) -> u16 {
        let hi = self.mem[self.pc as usize] as u16;
        let lo = self.mem[(self.pc + 1) as usize] as u16;
        (hi << 8) | lo
    }

    fn cycle(&mut self) -> std::result::Result<(), CpuError> {
        if let Some((x, snapshot)) = self.waiting_for_key {
            if let Some(k) = (0..16).find(|&k| self.keys[k] && !snapshot[k]) {
                self.v[x as usize] = k as u8;
                self.waiting_for_key = None;
            }
            return Ok(());
        }

        let opcode = self.fetch_opcode();
        self.pc += 2;
        self.execute(opcode)
    }

    fn execute(&mut self, opcode: u16) -> std::result::Result<(), CpuError> {
        let nnn = opcode & 0x0FFF;
        let nn = (opcode & 0x00FF) as u8;
        let n = (opcode & 0x000F) as u8;
        let x = ((opcode & 0x0F00) >> 8) as usize;
        let y = ((opcode & 0x00F0) >> 4) as usize;

        match opcode & 0xF000 {
            0x0000 => match opcode {
                0x00E0 => self.display = [0; (WIDTH as usize) * (HEIGHT as usize)],
                0x00EE => {
                    if self.sp == 0 {
                        return Err(CpuError::StackUnderflow);
                    }
                    self.sp -= 1;
                    self.pc = self.stack[self.sp as usize];
                }
                _ => {}
            },
            0x1000 => self.pc = nnn,
            0x2000 => {
                if self.sp as usize >= self.stack.len() {
                    return Err(CpuError::StackOverflow);
                }
                self.stack[self.sp as usize] = self.pc;
                self.sp += 1;
                self.pc = nnn;
            }
            0x3000 => {
                if self.v[x] == nn {
                    self.pc += 2;
                }
            }
            0x4000 => {
                if self.v[x] != nn {
                    self.pc += 2;
                }
            }
            0x5000 => {
                if self.v[x] == self.v[y] {
                    self.pc += 2;
                }
            }
            0x6000 => self.v[x] = nn,
            0x7000 => self.v[x] = self.v[x].wrapping_add(nn),
            0x8000 => match n {
                0x0 => self.v[x] = self.v[y],
                0x1 => self.v[x] |= self.v[y],
                0x2 => self.v[x] &= self.v[y],
                0x3 => self.v[x] ^= self.v[y],
                0x4 => {
                    let (res, carry) = self.v[x].overflowing_add(self.v[y]);
                    self.v[x] = res;
                    self.v[0xF] = carry as u8;
                }
                0x5 => {
                    let (res, borrow) = self.v[x].overflowing_sub(self.v[y]);
                    self.v[x] = res;
                    self.v[0xF] = (!borrow) as u8;
                }
                0x6 => {
                    let lsb = self.v[x] & 1;
                    self.v[x] >>= 1;
                    self.v[0xF] = lsb;
                }
                0x7 => {
                    let (res, borrow) = self.v[y].overflowing_sub(self.v[x]);
                    self.v[x] = res;
                    self.v[0xF] = (!borrow) as u8;
                }
                0xE => {
                    let msb = (self.v[x] >> 7) & 1;
                    self.v[x] <<= 1;
                    self.v[0xF] = msb;
                }
                _ => {}
            },
            0x9000 => {
                if self.v[x] != self.v[y] {
                    self.pc += 2;
                }
            }
            0xA000 => self.i = nnn,
            0xB000 => self.pc = nnn.wrapping_add(self.v[0] as u16),
            0xC000 => self.v[x] = rand::random::<u8>() & nn,
            0xD000 => {
                let px = self.v[x] as usize % WIDTH as usize;
                let py = self.v[y] as usize % HEIGHT as usize;
                self.v[0xF] = 0;
                for row in 0..n as usize {
                    let sprite = self.mem[(self.i as usize + row) % MEMORY_SIZE as usize];
                    for col in 0..8 {
                        if sprite & (0x80 >> col) != 0 {
                            let cx = (px + col) % WIDTH as usize;
                            let cy = (py + row) % HEIGHT as usize;
                            let idx = cy * WIDTH as usize + cx;
                            if self.display[idx] == 1 {
                                self.v[0xF] = 1;
                            }
                            self.display[idx] ^= 1;
                        }
                    }
                }
            }
            0xE000 => match nn {
                0x9E => {
                    if self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                0xA1 => {
                    if !self.keys[self.v[x] as usize] {
                        self.pc += 2;
                    }
                }
                _ => {}
            },
            0xF000 => match nn {
                0x07 => self.v[x] = self.delay_timer,
                0x0A => self.waiting_for_key = Some((x as u8, self.keys)),
                0x15 => self.delay_timer = self.v[x],
                0x18 => self.sound_timer = self.v[x],
                0x1E => self.i = self.i.wrapping_add(self.v[x] as u16),
                0x29 => {
                    let font_addr =
                        u16::from_str_radix(FONT_ADDR.trim_start_matches("0x"), 16).unwrap();
                    self.i = font_addr + (self.v[x] as u16) * 5;
                }
                0x33 => {
                    self.mem[self.i as usize] = self.v[x] / 100;
                    self.mem[self.i as usize + 1] = (self.v[x] / 10) % 10;
                    self.mem[self.i as usize + 2] = self.v[x] % 10;
                }
                0x55 => {
                    for k in 0..=x {
                        self.mem[self.i as usize + k] = self.v[k];
                    }
                }
                0x65 => {
                    for k in 0..=x {
                        self.v[k] = self.mem[self.i as usize + k];
                    }
                }
                _ => {}
            },
            _ => {}
        }

        Ok(())
    }

    fn update_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    fn set_key(&mut self, key: u8, pressed: bool) {
        self.keys[key as usize] = pressed;
    }

    fn set_keys(&mut self, window: &Window) {
        const KEY_MAP: [(Key, u8); 16] = [
            (Key::X, 0x0),
            (Key::Key1, 0x1),
            (Key::Key2, 0x2),
            (Key::Key3, 0x3),
            (Key::Q, 0x4),
            (Key::W, 0x5),
            (Key::E, 0x6),
            (Key::A, 0x7),
            (Key::S, 0x8),
            (Key::D, 0x9),
            (Key::Z, 0xA),
            (Key::C, 0xB),
            (Key::Key4, 0xC),
            (Key::R, 0xD),
            (Key::F, 0xE),
            (Key::V, 0xF),
        ];

        for (key, idx) in KEY_MAP {
            self.set_key(idx, window.is_key_down(key));
        }
    }

    fn render(&self, buffer: &mut [u32]) {
        for y in 0..HEIGHT as usize {
            for x in 0..WIDTH as usize {
                let color = if self.display[y * WIDTH as usize + x] != 0 {
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
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <rom>", args[0]);
        std::process::exit(1);
    }

    let mut cpu = CPU::new();
    if let Err(e) = cpu.load_rom(&args[1]) {
        eprintln!("Failed to load ROM '{}': {}", args[1], e);
        std::process::exit(1);
    }

    let mut window = Window::new("CHIP-8", WIN_W, WIN_H, WindowOptions::default())
        .expect("Failed to create window");
    window.limit_update_rate(Some(Duration::from_micros(16_600)));

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

        cpu.render(&mut buffer);
        window
            .update_with_buffer(&buffer, WIN_W, WIN_H)
            .expect("Failed to update window");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_with(opcodes: &[u16]) -> CPU {
        let mut cpu = CPU::new();
        let mut addr = 0x200usize;
        for op in opcodes {
            cpu.mem[addr] = (op >> 8) as u8;
            cpu.mem[addr + 1] = (op & 0x00FF) as u8;
            addr += 2;
        }
        cpu
    }

    #[test]
    fn fontset_is_loaded_at_font_addr() {
        let cpu = CPU::new();
        assert_eq!(cpu.mem[0x50], 0xF0);
        assert_eq!(cpu.mem[0x54], 0xF0);
        assert_eq!(cpu.mem[0x55], 0x20);
        assert_eq!(cpu.mem[0x56], 0x60);
    }

    #[test]
    fn set_register_and_advance_pc() {
        let mut cpu = cpu_with(&[0x60AB]);
        cpu.cycle().unwrap();
        assert_eq!(cpu.v[0], 0xAB);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn add_immediate_wraps() {
        let mut cpu = cpu_with(&[0x60FF, 0x7002]);
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        assert_eq!(cpu.v[0], 0x01);
    }

    #[test]
    fn add_registers_sets_carry() {
        let mut cpu = CPU::new();
        cpu.v[0] = 0xFF;
        cpu.v[1] = 0x02;
        cpu.execute(0x8014).unwrap();
        assert_eq!(cpu.v[0], 0x01);
        assert_eq!(cpu.v[0xF], 0x01);
    }

    #[test]
    fn subtract_registers_sets_borrow() {
        let mut cpu = CPU::new();
        cpu.v[0] = 0x02;
        cpu.v[1] = 0x03;
        cpu.execute(0x8015).unwrap();
        assert_eq!(cpu.v[0], 0xFF);
        assert_eq!(cpu.v[0xF], 0x00);
    }

    #[test]
    fn shift_right_stores_lsb() {
        let mut cpu = CPU::new();
        cpu.v[0] = 0b0000_0011;
        cpu.execute(0x8006).unwrap();
        assert_eq!(cpu.v[0], 0b0000_0001);
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn shift_left_stores_msb() {
        let mut cpu = CPU::new();
        cpu.v[0] = 0b1000_0001;
        cpu.execute(0x800E).unwrap();
        assert_eq!(cpu.v[0], 0b0000_0010);
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn jump_sets_pc() {
        let mut cpu = cpu_with(&[0x1234]);
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x234);
    }

    #[test]
    fn call_and_return_manage_stack() {
        let mut cpu = cpu_with(&[0x2204]);
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x204);
        assert_eq!(cpu.sp, 1);
        assert_eq!(cpu.stack[0], 0x202);

        cpu.mem[0x204] = 0x00;
        cpu.mem[0x205] = 0xEE;
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x202);
        assert_eq!(cpu.sp, 0);
    }

    #[test]
    fn stack_overflow_is_reported() {
        let mut cpu = cpu_with(&[0x2200]);
        for _ in 0..16 {
            cpu.cycle().unwrap();
        }
        assert_eq!(cpu.sp, 16);
        assert_eq!(cpu.cycle(), Err(CpuError::StackOverflow));
    }

    #[test]
    fn stack_underflow_is_reported() {
        let mut cpu = cpu_with(&[0x00EE]);
        assert_eq!(cpu.cycle(), Err(CpuError::StackUnderflow));
    }

    #[test]
    fn skip_if_equal_skips_next() {
        let mut cpu = cpu_with(&[0x600A, 0x300A, 0x1234]);
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x206);
    }

    #[test]
    fn skip_if_not_equal_does_not_skip() {
        let mut cpu = cpu_with(&[0x600A, 0x300B, 0x1234]);
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn font_address_opcode_points_at_glyph() {
        let mut cpu = CPU::new();
        cpu.v[0] = 1;
        cpu.execute(0xF029).unwrap();
        assert_eq!(cpu.i, 0x55);
        assert_eq!(cpu.mem[0x55], 0x20);
        assert_eq!(cpu.mem[0x56], 0x60);
    }

    #[test]
    fn bcd_encodes_decimal_digits() {
        let mut cpu = CPU::new();
        cpu.v[0] = 234;
        cpu.i = 0x300;
        cpu.execute(0xF033).unwrap();
        assert_eq!(cpu.mem[0x300], 2);
        assert_eq!(cpu.mem[0x301], 3);
        assert_eq!(cpu.mem[0x302], 4);
    }

    #[test]
    fn store_and_load_registers() {
        let mut cpu = CPU::new();
        cpu.v[0] = 1;
        cpu.v[1] = 2;
        cpu.v[2] = 3;
        cpu.i = 0x400;
        cpu.execute(0xF255).unwrap();
        assert_eq!(&cpu.mem[0x400..0x403], &[1, 2, 3]);

        cpu.v[0] = 0;
        cpu.v[1] = 0;
        cpu.v[2] = 0;
        cpu.execute(0xF265).unwrap();
        assert_eq!(&cpu.v[0..3], &[1, 2, 3]);
    }

    #[test]
    fn draw_sets_pixels_and_collision() {
        let mut cpu = CPU::new();
        cpu.mem[0x300] = 0xF0;
        cpu.i = 0x300;

        cpu.execute(0xD011).unwrap();
        assert_eq!(&cpu.display[0..4], &[1, 1, 1, 1]);
        assert_eq!(cpu.display[4], 0);
        assert_eq!(cpu.v[0xF], 0);

        cpu.execute(0xD011).unwrap();
        assert_eq!(&cpu.display[0..4], &[0, 0, 0, 0]);
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn draw_wraps_around_screen_edges() {
        let mut cpu = CPU::new();
        cpu.mem[0x300] = 0xC0;
        cpu.i = 0x300;
        cpu.v[0] = 63;
        cpu.v[1] = 0;
        cpu.execute(0xD011).unwrap();
        assert_eq!(cpu.display[0], 1);
        assert_eq!(cpu.display[63], 1);
    }

    #[test]
    fn clear_screen_resets_display() {
        let mut cpu = CPU::new();
        cpu.display[10] = 1;
        cpu.execute(0x00E0).unwrap();
        assert!(cpu.display.iter().all(|&p| p == 0));
    }

    #[test]
    fn fx0a_ignores_keys_already_held() {
        let mut cpu = CPU::new();
        cpu.set_key(5, true);
        cpu.execute(0xF30A).unwrap();
        assert!(cpu.waiting_for_key.is_some());

        cpu.cycle().unwrap();
        assert!(cpu.waiting_for_key.is_some());
        assert_eq!(cpu.v[3], 0);
    }

    #[test]
    fn fx0a_captures_new_key_press() {
        let mut cpu = CPU::new();
        cpu.set_key(5, true);
        cpu.execute(0xF30A).unwrap();

        cpu.set_key(5, false);
        cpu.set_key(7, true);
        cpu.cycle().unwrap();

        assert_eq!(cpu.v[3], 7);
        assert!(cpu.waiting_for_key.is_none());
    }

    #[test]
    fn fx0a_does_not_advance_pc_while_waiting() {
        let mut cpu = cpu_with(&[0xF30A, 0x1234]);
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x202);

        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x202);

        cpu.set_key(9, true);
        cpu.cycle().unwrap();
        assert_eq!(cpu.v[3], 9);
        assert_eq!(cpu.pc, 0x202);
    }

    #[test]
    fn delay_and_sound_timers_decrement() {
        let mut cpu = CPU::new();
        cpu.delay_timer = 2;
        cpu.sound_timer = 1;
        cpu.update_timers();
        assert_eq!(cpu.delay_timer, 1);
        assert_eq!(cpu.sound_timer, 0);
        cpu.update_timers();
        assert_eq!(cpu.delay_timer, 0);
        assert_eq!(cpu.sound_timer, 0);
    }

    #[test]
    fn random_opcode_respects_mask() {
        let mut cpu = CPU::new();
        cpu.execute(0xC000).unwrap();
        assert_eq!(cpu.v[0], 0);
    }
}
