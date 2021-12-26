use crate::{
    mmu::{Mmu},
    ProgramHeader,
    emulator::{
        Register::{Pc},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Register {
    Zero = 0,
    Ra,
    Sp,
    Gp,
    Tp,
    T0,
    T1,
    T2,
    S0,
    S1,
    A0,
    A1,
    A2,
    A3,
    A4,
    A5,
    A6,
    A7,
    S2,
    S3,
    S4,
    S5,
    S6,
    S7,
    S8,
    S9,
    S10,
    S11,
    T3,
    T4,
    T5,
    T6,
    Pc,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    regs: [usize; 33],
}

impl Default for State {
    fn default() -> Self {
        State {
            regs: [0; 33],
        }
    }
}

#[derive(Clone)]
pub struct Emulator {
    pub memory: Mmu,

    pub state: State,
}

impl Emulator {
    pub fn new(size: usize) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
        }
    }

    pub fn set_reg(&mut self, reg: Register, val: usize) {
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    pub fn get_reg(&self, reg: Register) -> usize {
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
    }

    pub fn load_section(&mut self, section: ProgramHeader, data: &[u8]) {
        self.memory.load_mem(section, data);
    }

    pub fn allocate(&mut self, size: usize) -> Option<usize>{
        self.memory.allocate(size)
    }

    pub fn free(&mut self, addr: usize) -> Option<()>{
        self.memory.free(addr)
    }

    pub fn run_emu(&mut self) {
        'next: loop {
            let pc = self.get_reg(Pc);

            if pc & 3 != 0 {
                panic!("Code unaligned");
            }

            //let instr = read_pc

            //match instr

            self.set_reg(Pc, pc.wrapping_add(4));
        }
    }
}
