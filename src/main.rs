use std::fs::File;
use std::io::{Read, Result};

const WIDTH: u8 = 64;
const HEIGHT: u8 = 32;
const MEMORY_SIZE: u16 = 4096;
const START_ADDR: &str = "0x200";
const FONT_ADDR: &str = "0x50";

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
    draw_flag: bool,
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
            draw_flag: false,
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
}

fn fetch_opcode(&self) -> u16 {
    let hi = self.mem[self.pc as usize] as u16;
    let lo = self.mem[(self.pc + 1) as usize] as u16;
    (hi << 8) | lo
}

fn main() {}
