use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    shared::Shared,
};

use std::sync::Arc;
use std::collections::VecDeque;

use iced_x86::code_asm::*;
use iced_x86::{Formatter, Instruction, NasmFormatter};
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
    /// rbp points to register array in memory
    /// r15 points to lookup array to check if pc is jitted
    fn compile(&mut self, pc: usize) -> Result<usize, IcedError> {
        //let func_size = self.function_map.get(&pc).unwrap();
        //let func_end = pc + func_size;
        let start_pc = pc;
        let mut asm = CodeAssembler::new(64).unwrap();
        let mut instr_queue = VecDeque::new();

        instr_queue.push_back(pc);

        // Load the base address of register array into rbp
        asm.mov(rbp, self.state.regs.as_ptr() as u64).unwrap();

        // Load the base address of jit lookup tableinto r15
        asm.mov(r15, self.shared.lookup.as_ptr() as u64).unwrap();

        while let Some(pc) = instr_queue.pop_front() {
            // If an error occurs during this read, it is most likely due to missing read or execute
            // permissions, so we mark it as an ExecFault
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();

            let instr = decode_instr(opcodes);

            match instr {
                Instr::Lui {rd, imm} => {
                    let rd_off = self.get_reg_offset(rd);
                    let sign_extended = imm as i64 as u64;
                    asm.mov(rax, sign_extended).unwrap();
                    asm.mov(ptr(rbp+rd_off), rax).unwrap();
                },
                Instr::Auipc {rd, imm} => {
                    let rd_off = self.get_reg_offset(rd);
                    let sign_extended = (imm + pc as i32) as i64 as u64;
                    asm.mov(rax, sign_extended).unwrap();
                    asm.mov(ptr(rbp+rd_off), rax).unwrap();
                },
                Instr::Jal {rd, imm} => {
                    let rd_off = self.get_reg_offset(rd);
                    let _pc_off = self.get_reg_offset(Register::Pc);
                    let ret_val = (pc + 4) as u64;
                    let jmp_target = pc + imm as usize;

                    let mut jit_exit = asm.create_label();

                    // Jump without return so can just emit code at that location
                    if rd == Register::Zero {
                       instr_queue.push_back(jmp_target);
                       continue;
                    }

                    // Move pc+4 into rd
                    asm.mov(rax, ret_val).unwrap();
                    asm.mov(rbp+rd_off, rax).unwrap();

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + jmp_target)).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap(); //(not in jit).unwrap();
                    asm.jmp(rax).unwrap();

                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.mov(rbx, jmp_target as u64).unwrap();
                    asm.ret().unwrap();
                },
                Instr::Jalr {rd, imm, rs1} => {
                    let rd_off = self.get_reg_offset(rd);
                    let rs1_off = self.get_reg_offset(rs1);
                    let _pc_off = self.get_reg_offset(Register::Pc);
                    let ret_val = (pc + 4) as u64;

                    let mut jit_exit = asm.create_label();

                    // Move pc+4 into rd
                    asm.mov(rax, ret_val).unwrap();
                    asm.mov(rbp+rd_off, rax).unwrap();

                    // Move jump target into rcx
                    asm.mov(rcx, ptr(rbp+rs1_off)).unwrap();
                    asm.add(rcx, imm).unwrap();

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + rcx)).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap(); //(not in jit).unwrap();
                    asm.jmp(rax).unwrap();

                    // exit jit
                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.mov(rbx, rcx).unwrap();
                    asm.ret().unwrap();
                },
                Instr::Beq  { rs1, rs2, imm, mode } |
                Instr::Bne  { rs1, rs2, imm, mode } |
                Instr::Blt  { rs1, rs2, imm, mode } |
                Instr::Bge  { rs1, rs2, imm, mode } |
                Instr::Bltu { rs1, rs2, imm, mode } |
                Instr::Bgeu { rs1, rs2, imm, mode } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    let jmp_target = pc + imm as usize;
                    let mut jit_exit = asm.create_label();
                    let mut fallthrough = asm.create_label();

                    asm.mov(rax, ptr(rbp+rs1_off)).unwrap();
                    asm.cmp(rax, ptr(rbp+rs2_off)).unwrap();
                    match mode {
                        0b000 => { asm.je(fallthrough).unwrap();  },
                        0b001 => { asm.jne(fallthrough).unwrap(); },
                        0b100 => { asm.jmp(fallthrough).unwrap(); },
                        0b101 => { asm.jge(fallthrough).unwrap(); },
                        0b110 => { asm.jmp(fallthrough).unwrap(); },
                        0b111 => { asm.jmp(fallthrough).unwrap(); },
                        _ => { unreachable!(); },
                    }

                    // Move jump target into rcx
                    asm.mov(rcx, jmp_target as u64).unwrap();

                    // Check if addr is in jit
                    asm.mov(rax, ptr(r15 + rcx)).unwrap();
                    asm.test(rax, rax).unwrap();
                    asm.jz(jit_exit).unwrap(); //(not in jit).unwrap();
                    asm.jmp(rax).unwrap();

                    // exit jit
                    asm.set_label(&mut jit_exit).unwrap();
                    asm.mov(rax, 1u64).unwrap();
                    asm.mov(rbx, rcx).unwrap();
                    asm.ret().unwrap();

                    // Fall through to next instruction
                    asm.set_label(&mut fallthrough).unwrap();
                }
                Instr::Add {rd, rs1, rs2 } => {
                    let rs1_off = self.get_reg_offset(rs1);
                    let rs2_off = self.get_reg_offset(rs2);
                    let rd_off  = self.get_reg_offset(rd);
                    asm.mov(rax, ptr(rbp+rs1_off)).unwrap();
                    asm.mov(rbx, ptr(rbp+rs2_off)).unwrap();
                    asm.add(rax, rbx).unwrap();
                    asm.mov(ptr(rbp+rd_off), rax).unwrap();
                },
                Instr::Addi {rd, rs1, imm } => {
                    if rd == Register::Zero && rs1 == Register::Zero && imm == 0 {
                        // Nop
                    } else {
                        let rs1_off = self.get_reg_offset(rs1);
                        let rd_off  = self.get_reg_offset(rd);
                        asm.mov(rax, ptr(rbp+rs1_off)).unwrap();
                        if imm != 0 {
                            asm.add(rax, imm).unwrap();
                        }
                        asm.mov(ptr(rbp+rd_off), rax).unwrap();
                    }
                },
                _ => { self.dump_instrs(asm.instructions(), start_pc, pc); },
            }
            instr_queue.push_back(pc + 4);
        }

        // Add code into JIT and return address of this jit-block
        Ok((*self.shared).add_jitblock(&asm.assemble(0x0)?, start_pc))
    }

    fn dump_instrs(&self, instrs: &[Instruction], mut pc: usize, end_pc: usize) -> ! {
        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                    Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);
            println!("0x{:x}: {:?}", pc, instr);
            pc+=4;
        }

        for instr in instrs {
            let mut formatter = NasmFormatter::new();
            let mut output = String::new();

            output.clear();
            formatter.format(&instr, &mut output);
            println!("{:#?}", output);
        }
        panic!("Unimplemented Instruction hit");
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary() {
        let shared = Arc::new(Shared::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(1024 * 1024, shared);

        let addr = emu.allocate(0x40, Perms::READ | Perms::WRITE | Perms::EXECUTE).unwrap();
        emu.set_reg(Register::Pc, addr);

        let data = std::fs::read("tests/output").unwrap();
        emu.memory.write_mem(addr, &data, data.len()).unwrap();

        println!("size: {}", data.len());
        emu.run_jit();
    }
}


