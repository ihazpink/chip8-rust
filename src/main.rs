extern crate minifb;
extern crate rand;
use minifb::{Key, KeyRepeat, Window, WindowOptions};

fn read_word(memory: [u8; 4096], index: u16) -> u16 {
    (memory[index as usize] as u16) << 8 | (memory[(index + 1) as usize] as u16)
}

const WIDTH: usize = 64;
const HEIGHT: usize = 64;
const MEMORY_SIZE: u16 = 4096;

struct Cpu {
    pub i: u16,  // Index register
    pub pc: u16, // Program counter
    pub memory: [u8; MEMORY_SIZE as usize],
    pub v: [u8; 16], // Registers
    pub keypad: [bool; 16],
    pub display: [bool; WIDTH * HEIGHT],
    pub stack: [u16; 16],
    pub sp: u8, // Stack pointer
    pub dt: u8, // Delay timer
    pub st: u8, // Sound timer
    pub cycle_count: u8,
}

impl Cpu {
    const FONTSET: [u8; 80] = [
        0xF0, 0x90, 0x90, 0x90, 0xF0, // 0
        0x20, 0x60, 0x20, 0x20, 0x70, // 1
        0xF0, 0x10, 0xF0, 0x80, 0xF0, // 2
        0xF0, 0x10, 0xF0, 0x10, 0xF0, // 3
        0x90, 0x90, 0xF0, 0x10, 0x10, // 4
        0xF0, 0x80, 0xF0, 0x10, 0xF0, // 5
        0xF0, 0x80, 0xF0, 0x90, 0xF0, // 6
        0xF0, 0x10, 0x20, 0x40, 0x40, // 7
        0xF0, 0x90, 0xF0, 0x90, 0xF0, // 8
        0xF0, 0x90, 0xF0, 0x10, 0xF0, // 9
        0xF0, 0x90, 0xF0, 0x90, 0x90, // A
        0xE0, 0x90, 0xE0, 0x90, 0xE0, // B
        0xF0, 0x80, 0x80, 0x80, 0xF0, // C
        0xE0, 0x90, 0x90, 0x90, 0xE0, // D
        0xF0, 0x80, 0xF0, 0x80, 0xF0, // E
        0xF0, 0x80, 0xF0, 0x80, 0x80, // F
    ];

    pub fn initialize(&mut self) {
        // Sets pc to default
        // Clears opcode, index register and stack pointer
        self.pc = 0x200;
        self.i = 0;
        self.sp = 0;

        // Clears display, memory, registers and stack
        self.display = [false; WIDTH * HEIGHT];
        self.stack = [0; 16];
        self.v = [0; 16];
        self.memory = [0; MEMORY_SIZE as usize];

        // Loads fontset
        for i in 0..80 {
            self.memory[i] = Cpu::FONTSET[i];
        }

        // Resets timers
        self.dt = 0;
        self.st = 0;
    }

    fn draw(&mut self, height: u16, vx: u16, vy: u16) {
        self.v[0xF] = 0;
        for i in 0..height {
            for j in 0..8 {
                let index = ((i + vy) % HEIGHT as u16) * WIDTH as u16 + ((vx + j) % WIDTH as u16);
                let cur_pix = self.display[index as usize];
                let new_pix =
                    self.memory[((self.i + i) as u16 % MEMORY_SIZE) as usize] & (0x80 >> j);

                if new_pix != 0 {
                    if cur_pix == true {
                        self.v[0xF] = 1;
                    }
                    self.display[index as usize] ^= true;
                }
            }
        }
    }

    fn op_fx33(&mut self, vx: u8) {
        self.memory[self.i as usize] = vx / 100;
        self.memory[((self.i + 1) % MEMORY_SIZE) as usize] = (vx / 10) % 10;
        self.memory[((self.i + 2) % MEMORY_SIZE) as usize] = vx % 10;
    }

    fn op_fx55(&mut self, x: u16) {
        for offset in 0..(x + 1) {
            self.memory[((self.i + offset) % MEMORY_SIZE) as usize] = self.v[offset as usize]
        }
    }

    fn op_fx65(&mut self, x: u16) {
        for offset in 0..(x + 1) {
            self.v[offset as usize] = self.memory[(self.i + offset) as usize]
        }
    }

    pub fn load_rom(&mut self, file_path: String) {
        let rom = std::fs::read(file_path).expect("rom failed to load");
        for i in 0..rom.len() {
            self.memory[i + 0x200] = rom[i];
        }
    }

    pub fn emulate_cycle(&mut self) {
        let opcode = read_word(self.memory, self.pc);
        self.pc += 2;
        self.cycle_count = (self.cycle_count + 1) % 16;

        let op1 = opcode >> 12; // First hex
        let op2 = (opcode & 0x0F00) >> 8; // Second hex
        let op3 = (opcode & 0x00F0) >> 4; // Third hex
        let op4 = opcode & 0x000F; // Last hex

        let x = ((opcode & 0x0F00) >> 8) as usize;
        let y = ((opcode & 0x00F0) >> 4) as usize;
        let vx = self.v[x];
        let vy = self.v[y];
        let nnn = opcode & 0x0FFF;
        let kk = (opcode & 0x00FF) as u8;

        self.i %= MEMORY_SIZE;

        match (op1, op2, op3, op4) {
            // 00E0 CLS
            (0, 0, 0xE, 0) => self.display = [false; WIDTH * HEIGHT],
            // 00EE RET
            (0, 0, 0xE, 0xE) => {
                self.pc = self.stack[self.sp as usize];
                self.sp = (self.sp - 1) % 16;
            }
            // 1nnn JP
            (1, _, _, _) => self.pc = nnn,
            // 2nnn CALL
            (2, _, _, _) => {
                self.sp = (self.sp + 1) % 16;
                self.stack[self.sp as usize] = self.pc;
                self.pc = nnn;
            }
            // 3xkk SE
            (3, _, _, _) => {
                if vx == kk {
                    self.pc += 2
                }
            }
            // 4xkk SNE
            (4, _, _, _) => {
                if vx != kk {
                    self.pc += 2
                }
            }
            // 5xy0 SE
            (5, _, _, 0) => {
                if vx == vy {
                    self.pc += 2
                }
            }
            // 6xkk LD
            (6, _, _, _) => {
                self.v[x] = kk;
            }
            // 7xkk ADD
            (7, _, _, _) => self.v[x] = self.v[x].overflowing_add(kk).0,
            //8xy0 LD
            (8, _, _, 0) => self.v[x] = vy,
            //8xy1 OR
            (8, _, _, 1) => self.v[x] |= vy,
            //8xy2 AND
            (8, _, _, 2) => self.v[x] &= vy,
            //8xy3 XOR
            (8, _, _, 3) => self.v[x] ^= vy,
            //8xy4 ADD
            (8, _, _, 4) => {
                self.v[0xF] = ((vx as u16).overflowing_add(vy as u16).0 > 255) as u8;
                self.v[x] = vx.overflowing_add(vy).0
            }
            //8xy5 SUB
            (8, _, _, 5) => {
                self.v[0xF] = (vx > vy) as u8;
                self.v[x] = vx.overflowing_sub(vy).0
            }
            //8xy6 SHR
            (8, _, _, 6) => {
                self.v[0xF] = vx & 1;
                self.v[x] >>= 1;
            }
            //8xy7 SUBN
            (8, _, _, 7) => {
                self.v[0xF] = (vy > vx) as u8;
                self.v[x] = vy.overflowing_sub(vx).0
            }
            //8xyE SHL
            (8, _, _, 0xE) => {
                self.v[0xF] = (vx >> 7) & 1;
                self.v[x] <<= 1;
            }
            //9xy0 SNE
            (9, _, _, 0) => {
                if vx != vy {
                    self.pc += 2
                }
            }
            //Annn LD
            (0xA, _, _, _) => {
                self.i = nnn;
            }
            //Bnnn JP
            (0xB, _, _, _) => self.pc = nnn + self.v[0] as u16,
            //Cxkk RND
            (0xC, _, _, _) => self.v[x] = rand::random::<u8>() & kk as u8,
            //Dxyn DRW
            (0xD, _, _, _) => self.draw(op4, vx as u16, vy as u16),
            //Ex9E SKP Vx
            (0xE, _, 9, 0xE) => {
                if self.keypad[vx as usize] {
                    self.pc += 2
                }
            }
            //ExA1 SKNP Vx
            (0xE, _, 0xA, 1) => {
                if self.keypad[vx as usize] {
                    self.pc += 2
                }
            }
            //Fx07 DT
            (0xF, _, 0, 7) => self.v[x] = self.dt,
            //Fx0A
            (0xF, _, 0, 0xA) => {}
            //Fx15
            (0xF, _, 1, 5) => self.dt = vx,
            //Fx18
            (0xF, _, 1, 8) => self.st = vx,
            //Fx1E
            (0xF, _, 1, 0xE) => self.i += vx as u16,
            //Fx29
            (0xF, _, 2, 9) => self.i = (vx * 5) as u16,
            //Fx33
            (0xF, _, 3, 3) => self.op_fx33(self.v[x]),
            //Fx55
            (0xF, _, 5, 5) => self.op_fx55(op2),
            //Fx65
            (0xF, _, 6, 5) => self.op_fx65(op2),
            (_, _, _, _) => println!("Unknown opcode: {:#0x}", opcode),
        }
        if (self.cycle_count + 1) == 16 && self.dt > 0 {
            self.dt = self.dt.wrapping_sub(1);
        }
    } // end of emulate cycle
} // end of impl cpu

impl Default for Cpu {
    fn default() -> Self {
        Cpu {
            i: 0,
            pc: 0,
            memory: [0; 4096],
            v: [0; 16],
            keypad: [false; 16],
            display: [false; WIDTH * HEIGHT],
            stack: [0; 16],
            sp: 0,
            dt: 0,
            st: 0,
            cycle_count: 0,
        }
    }
}

fn main() {
    let mut chip8: Cpu = Default::default();
    let mut buffer: [u32; WIDTH * HEIGHT] = [0; WIDTH * HEIGHT];
    let keypad = vec![
        Key::Key1,
        Key::Key2,
        Key::Key3,
        Key::Key4,
        Key::Q,
        Key::W,
        Key::E,
        Key::R,
        Key::A,
        Key::S,
        Key::D,
        Key::F,
        Key::Z,
        Key::X,
        Key::C,
        Key::V,
    ];

    chip8.initialize();

    //chip8.load_rom(_test5_path);

    let window_opts = WindowOptions {
        scale: minifb::Scale::X8,
        scale_mode: minifb::ScaleMode::Stretch,
        ..WindowOptions::default()
    };

    let mut window =
        Window::new("Test - ESC to exit", WIDTH, HEIGHT, window_opts).unwrap_or_else(|e| {
            panic!("{}", e);
        });

    window.limit_update_rate(Some(std::time::Duration::from_millis(1)));

    while window.is_open() && !window.is_key_down(Key::Escape) && chip8.pc != 0xFFF {
        for i in 0..chip8.display.len() {
            if chip8.display[i] == false {
                buffer[i] = 0
            } else {
                buffer[i] = u32::MAX
            }
        }
        let pressed_keys = window.get_keys_pressed(KeyRepeat::Yes);

        if pressed_keys.is_some() {
            let pressed_keys = pressed_keys.unwrap();
            for i in 0..keypad.len() {
                if pressed_keys.contains(&keypad[i]) {
                    chip8.keypad[i] = true
                }
            }
        }

        chip8.emulate_cycle();
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }
}
