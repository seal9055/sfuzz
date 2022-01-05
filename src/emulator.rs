use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    shared::Shared,
};

use std::sync::Arc;
use std::collections::VecDeque;

use iced_x86::code_asm::*;
use rustc_hash::FxHashMap;

/// 33 RISCV Registers
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

impl From<u32> for Register {
    fn from(val: u32) -> Self {
        assert!(val < 33);
        unsafe {
            core::ptr::read_unaligned(&(val as usize) as *const usize as *const Register)
        }
    }
}

/// Various faults that can occur during program execution. These can be syscalls, bugs, or other
/// non-standard behaviors that require kernel involvement
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fault {
    /// Syscall
    Syscall,

    /// Fault occurs when an attempt is made to write to an address without Perms::WRITE set
    WriteFault(usize),

    /// Fault occurs when an attempt is made to read from an address without Perms::READ set
    ReadFault(usize),

    /// Fault occurs when an attempt is made to execute an invalid instruction
    ExecFault(usize),

    /// Fault occurs when some operation results in an integer overflow
    IntegerOverflow,

    /// Fault occurs when a free fails. Occurs when an invalid address is attempted to be free'd
    /// or when a free is used on a chunk of memory that does not contain the Perms::ISALLOC
    /// permission in its metadata
    InvalidFree(usize),
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

// TODO remove clone, should only be arc cloned
#[derive(Clone)]
pub struct Emulator {
    pub memory: Mmu,

    pub state: State,

    pub function_map: FxHashMap<usize, usize>,

    pub shared: Arc<Shared>,
}

impl Emulator {
    pub fn new(size: usize, shared: Arc<Shared>) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
            function_map: FxHashMap::default(),
            shared,
        }
    }

    pub fn set_reg(&mut self, reg: Register, val: usize) {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    pub fn get_reg(&self, reg: Register) -> usize {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
    }

    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        self.memory.load_segment(segment, data)
    }

    pub fn allocate(&mut self, size: usize, perms: u8) -> Option<usize> {
        self.memory.allocate(size, perms)
    }

    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        self.memory.free(addr)
    }

    pub fn run_jit(&mut self) -> Option<Fault> {
        //loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Some(Fault::ExecFault(pc)); }

            //let jit_addr: usize = (*self.shared).lookup(pc);
            let jit_addr: usize = self.compile(pc).unwrap();

            println!("jit function address = {:x}", jit_addr);

            //unsafe {
            //    let func = *(&jit_addr as *const usize as *const fn());
            //    func();
            //}

            None
        //}
    }

    fn get_reg_offset(&self, reg: Register) -> usize {
        reg as usize * 8
    }

    /// JIT compile a function
    fn compile(&mut self, mut pc: usize) -> Result<usize, IcedError> {
        //let func_size = self.function_map.get(&pc).unwrap();
        //let func_end = pc + func_size;
        let mut asm = CodeAssembler::new(64).unwrap();
        let mut instr_queue = VecDeque::new();

        instr_queue.push_back(pc);

        while let Some(pc) = instr_queue.pop_front() {
            // If an error occurs during this read, it is most likely due to missing read or execute
            // permissions, so we mark it as an ExecFault
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();

            let instr = decode_instr(opcodes);

            // Load the base address of register array into rbp
            asm.mov(rbp, self.state.regs.as_ptr() as u64)?;

            match instr {
                Instr::Jal {rd, imm} => {
                    let off1 = self.get_reg_offset(rd);
                    let off2 = self.get_reg_offset(Register::Pc);
                    let ret_val = (pc + 4) as u32;
                    let jmp_target = pc + imm as usize;

                    // Link the return address into rd
                    asm.mov(ptr(rbp+off1), ret_val)?;

                    if rd == Register::Zero {
                        instr_queue.push_back(jmp_target);
                    }

                    // attempt to translate jump target
                      // if success
                        // emit jmp instr
                      // else if failure,
                        // if diff function
                          // if size < 200
                            // compile the function at target and then emit jmp instr
                          // else if size > 200
                            // JITEXIT
                        // else if same function
                          // IDK (labels?)
                          // Create label, and set a jump to it. Later when the address is actually
                          // hit, place the label

                    asm.mov(ptr(rbp+off1), ret_val)?;

                    if rd == Register::Zero {
                        // Direct Jump
                        //asm.jmp();
                        instr_queue.push_back(jmp_target);
                    } else {
                        // Function call
                    }

                    //asm.jmp(jump_target as u64)?;

                    //
                },
                Instr::Lui {rd, imm} => {
                    let off1 = self.get_reg_offset(rd);
                    asm.mov(ptr(rbp+off1), imm)?;
                },
                Instr::Add {rd, rs1, rs2 } => {
                    let off1 = self.get_reg_offset(rs1);
                    let off2 = self.get_reg_offset(rs2);
                    let off3 = self.get_reg_offset(rd);
                    asm.mov(rax, ptr(rbp+off1))?;
                    asm.mov(rbx, ptr(rbp+off2))?;
                    asm.add(rax, rbx)?;
                    asm.mov(ptr(rbp+off3), rax)?;
                },
                Instr::Addi {rd, rs1, imm } => {
                    let off1 = self.get_reg_offset(rs1);
                    let off2 = self.get_reg_offset(rd);
                    asm.mov(rax, ptr(rbp+off1))?;
                    asm.add(rax, imm)?;
                    asm.mov(ptr(rbp+off2), rax)?;
                },
                _ => {},
            }
            instr_queue.push_back(pc + 4);
        }

        // Add code into JIT and return address of this jit-block
        Ok((*self.shared).add_jitblock(&asm.assemble(0x0)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary() {
        let shared = Arc::new(Shared::new());
        let mut emu = Emulator::new(1024 * 1024, shared);

        let addr = emu.allocate(0x40, Perms::READ | Perms::WRITE | Perms::EXECUTE).unwrap();
        emu.set_reg(Register::Pc, addr);

        let data = std::fs::read("tests/output").unwrap();
        emu.memory.write_mem(addr, &data, data.len()).unwrap();

        println!("size: {}", data.len());
        emu.run_jit();
    }
}


