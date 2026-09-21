use std::fs::File;
use std::io::{Read, Result};

use minifb::{Key, Window};

pub const WIDTH: usize = 64;
pub const HEIGHT: usize = 32;

const MEMORY_SIZE: usize = 4096;
const START_ADDR: u16 = 0x200;
const FONT_ADDR: u16 = 0x50;

#[derive(Debug, PartialEq, Eq)]
pub enum CpuError {
    StackOverflow,
    StackUnderflow,
}

pub struct Cpu {
    mem: [u8; MEMORY_SIZE],
    v: [u8; 16],
    i: u16,
    pc: u16,
    stack: [u16; 16],
    sp: u8,
    delay_timer: u8,
    sound_timer: u8,
    display: [u8; WIDTH * HEIGHT],
    keys: [bool; 16],
    // When `Some((x, snapshot))`, the CPU is blocked on an `FX0A` instruction.
    // `x` is the target register and `snapshot` is the key state at the moment
    // `FX0A` executed, so only a *newly* pressed key is accepted.
    waiting_for_key: Option<(u8, [bool; 16])>,
}

impl Cpu {
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

        let mut mem = [0u8; MEMORY_SIZE];
        mem[FONT_ADDR as usize..FONT_ADDR as usize + FONTSET.len()].copy_from_slice(&FONTSET);

        Cpu {
            mem,
            v: [0; 16],
            i: 0,
            pc: START_ADDR,
            stack: [0; 16],
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            display: [0; WIDTH * HEIGHT],
            keys: [false; 16],
            waiting_for_key: None,
        }
    }

    pub fn load_rom(&mut self, path: &str) -> Result<()> {
        let mut file = File::open(path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let start = START_ADDR as usize;
        let max_size = MEMORY_SIZE - start;
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

    pub fn cycle(&mut self) -> std::result::Result<(), CpuError> {
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

    pub fn update_timers(&mut self) {
        if self.delay_timer > 0 {
            self.delay_timer -= 1;
        }
        if self.sound_timer > 0 {
            self.sound_timer -= 1;
        }
    }

    pub fn set_keys(&mut self, window: &Window) {
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

    pub fn display(&self) -> &[u8] {
        &self.display
    }

    fn set_key(&mut self, key: u8, pressed: bool) {
        self.keys[key as usize] = pressed;
    }

    fn fetch_opcode(&self) -> u16 {
        let hi = self.mem[self.pc as usize] as u16;
        let lo = self.mem[(self.pc + 1) as usize] as u16;
        (hi << 8) | lo
    }

    fn execute(&mut self, opcode: u16) -> std::result::Result<(), CpuError> {
        let nnn = opcode & 0x0FFF;
        let nn = (opcode & 0x00FF) as u8;
        let n = (opcode & 0x000F) as u8;
        let x = ((opcode & 0x0F00) >> 8) as usize;
        let y = ((opcode & 0x00F0) >> 4) as usize;

        match opcode & 0xF000 {
            0x0000 => match opcode {
                0x00E0 => self.display = [0; WIDTH * HEIGHT],
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
                let px = self.v[x] as usize % WIDTH;
                let py = self.v[y] as usize % HEIGHT;
                self.v[0xF] = 0;
                for row in 0..n as usize {
                    let sprite = self.mem[(self.i as usize + row) % MEMORY_SIZE];
                    for col in 0..8 {
                        if sprite & (0x80 >> col) != 0 {
                            let cx = (px + col) % WIDTH;
                            let cy = (py + row) % HEIGHT;
                            let idx = cy * WIDTH + cx;
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
                0x29 => self.i = FONT_ADDR + (self.v[x] as u16) * 5,
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cpu_with(opcodes: &[u16]) -> Cpu {
        let mut cpu = Cpu::new();
        let mut addr = START_ADDR as usize;
        for op in opcodes {
            cpu.mem[addr] = (op >> 8) as u8;
            cpu.mem[addr + 1] = (op & 0x00FF) as u8;
            addr += 2;
        }
        cpu
    }

    #[test]
    fn fontset_is_loaded_at_font_addr() {
        let cpu = Cpu::new();
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
        let mut cpu = Cpu::new();
        cpu.v[0] = 0xFF;
        cpu.v[1] = 0x02;
        cpu.execute(0x8014).unwrap();
        assert_eq!(cpu.v[0], 0x01);
        assert_eq!(cpu.v[0xF], 0x01);
    }

    #[test]
    fn subtract_registers_sets_borrow() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x02;
        cpu.v[1] = 0x03;
        cpu.execute(0x8015).unwrap();
        assert_eq!(cpu.v[0], 0xFF);
        assert_eq!(cpu.v[0xF], 0x00);
    }

    #[test]
    fn subtract_reverse_registers() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x02;
        cpu.v[1] = 0x05;
        cpu.execute(0x8017).unwrap();
        assert_eq!(cpu.v[0], 0x03);
        assert_eq!(cpu.v[0xF], 0x01);
    }

    #[test]
    fn shift_right_stores_lsb() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0b0000_0011;
        cpu.execute(0x8006).unwrap();
        assert_eq!(cpu.v[0], 0b0000_0001);
        assert_eq!(cpu.v[0xF], 1);
    }

    #[test]
    fn shift_left_stores_msb() {
        let mut cpu = Cpu::new();
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
    fn jump_with_offset_uses_v0() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 0x05;
        cpu.execute(0xB300).unwrap();
        assert_eq!(cpu.pc, 0x305);
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
    fn skip_if_registers_equal() {
        let mut cpu = cpu_with(&[0x6005, 0x6105, 0x5010, 0x1234]);
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x208);
    }

    #[test]
    fn skip_if_registers_not_equal() {
        let mut cpu = cpu_with(&[0x6005, 0x6106, 0x9010, 0x1234]);
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x208);
    }

    #[test]
    fn skip_if_key_pressed() {
        let mut cpu = cpu_with(&[0xE09E, 0x1234]);
        cpu.set_key(0, true);
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn skip_if_key_not_pressed() {
        let mut cpu = cpu_with(&[0xE0A1, 0x1234]);
        cpu.cycle().unwrap();
        assert_eq!(cpu.pc, 0x204);
    }

    #[test]
    fn font_address_opcode_points_at_glyph() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 1;
        cpu.execute(0xF029).unwrap();
        assert_eq!(cpu.i, 0x55);
        assert_eq!(cpu.mem[0x55], 0x20);
        assert_eq!(cpu.mem[0x56], 0x60);
    }

    #[test]
    fn bcd_encodes_decimal_digits() {
        let mut cpu = Cpu::new();
        cpu.v[0] = 234;
        cpu.i = 0x300;
        cpu.execute(0xF033).unwrap();
        assert_eq!(cpu.mem[0x300], 2);
        assert_eq!(cpu.mem[0x301], 3);
        assert_eq!(cpu.mem[0x302], 4);
    }

    #[test]
    fn store_and_load_registers() {
        let mut cpu = Cpu::new();
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
        let mut cpu = Cpu::new();
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
    fn draw_wraps_around_right_edge() {
        let mut cpu = Cpu::new();
        cpu.mem[0x300] = 0xC0;
        cpu.i = 0x300;
        cpu.v[0] = 63;
        cpu.v[1] = 0;
        cpu.execute(0xD011).unwrap();
        assert_eq!(cpu.display[0], 1);
        assert_eq!(cpu.display[63], 1);
    }

    #[test]
    fn draw_wraps_around_bottom_edge() {
        let mut cpu = Cpu::new();
        cpu.mem[0x300] = 0x80;
        cpu.mem[0x301] = 0x80;
        cpu.i = 0x300;
        cpu.v[0] = 0;
        cpu.v[1] = 31;
        cpu.execute(0xD012).unwrap();
        assert_eq!(cpu.display[31 * WIDTH], 1);
        assert_eq!(cpu.display[0], 1);
    }

    #[test]
    fn clear_screen_resets_display() {
        let mut cpu = Cpu::new();
        cpu.display[10] = 1;
        cpu.execute(0x00E0).unwrap();
        assert!(cpu.display.iter().all(|&p| p == 0));
    }

    #[test]
    fn fx0a_ignores_keys_already_held() {
        let mut cpu = Cpu::new();
        cpu.set_key(5, true);
        cpu.execute(0xF30A).unwrap();
        assert!(cpu.waiting_for_key.is_some());

        cpu.cycle().unwrap();
        assert!(cpu.waiting_for_key.is_some());
        assert_eq!(cpu.v[3], 0);
    }

    #[test]
    fn fx0a_captures_new_key_press() {
        let mut cpu = Cpu::new();
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
        let mut cpu = Cpu::new();
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
        let mut cpu = Cpu::new();
        cpu.execute(0xC000).unwrap();
        assert_eq!(cpu.v[0], 0);
    }
}
