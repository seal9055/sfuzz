use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    jit::Jit,
    syscalls,
    error_exit,
    irgraph::{IRGraph, Flag},
};

use std::sync::Arc;
use std::arch::asm;
use std::collections::BTreeMap;

use rustc_hash::FxHashMap;
use iced_x86::code_asm::*;

pub const STDIN:  isize = 0;
pub const STDOUT: isize = 1;
pub const STDERR: isize = 2;

/// 33 RISCV Registers + 2 Extra temporary registers that the IR needs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
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
    Z1,
    Z2
}

impl Register {
    pub fn is_spilled(&self) -> bool {
        true
    }

    pub fn convert_64(&self) -> AsmRegister64 {
        rax
    }

    pub fn convert_32(&self) -> AsmRegister32 {
        eax
    }

    pub fn get_offset(&self) -> u64 {
        *self as u64 * 8
    }
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

    /// Fault occurs when there is no more room to service new allocations
    OOM,

    /// Process called exit
    Exit,
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

/// Emulator that runs the actual code. Each thread gets its own emulator in which everything is
/// separate except the jit backing that all emulators share.
#[derive(Clone)]
pub struct Emulator {
    /// Memory backing for the emulator, contains actual memory bytes and permissions
    pub memory: Mmu,

    /// Describes the current state of the emulator
    pub state: State,

    /// These are used to hook specific addresses. Can be used for debug purposes or to redirect
    /// important functions such as malloc/free to our own custom implementations
    pub hooks: FxHashMap<usize, fn(&mut Emulator) -> Result<(), Fault>>,

    /// List of file descriptors that the process can use for syscalls
    pub fd_list: Vec<isize>,

    /// Mapping between function address and function size
    pub functions: FxHashMap<usize, usize>,

    /// The actual jit compiler backing
    pub jit: Arc<Jit>,
}

impl Emulator {
    /// Create a new emulator that has access to the shared jit backing
    pub fn new(size: usize, jit: Arc<Jit>) -> Self {
        Emulator {
            memory:  Mmu::new(size),
            state:   State::default(),
            hooks:   FxHashMap::default(),
            fd_list: vec![STDIN, STDOUT, STDERR],
            functions: FxHashMap::default(),
            jit,
        }
    }

    /// Fork an emulator into a new one, basically creating an exact copy
    #[must_use]
    pub fn fork(&self) -> Self {
        Emulator {
            memory:  self.memory.fork(),
            state:   State::default(),
            hooks:   self.hooks.clone(),
            fd_list: self.fd_list.clone(),
            functions:  self.functions.clone(),
            jit:     self.jit.clone(),
        }
    }

    /// Reset the entire state of this emulator (memory & registers)
    pub fn reset(&mut self, original: &Self) {
        self.memory.reset(&original.memory);
        self.state.regs = original.state.regs;

        self.fd_list.clear();
        self.fd_list.extend_from_slice(&original.fd_list);
    }

    /// Set a register
    pub fn set_reg(&mut self, reg: Register, val: usize) {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { panic!("Can't set zero-register"); }
        self.state.regs[reg as usize] = val;
    }

    /*
    /// Set a register using assembly instructions in the jit
    pub fn jit_set_reg(&mut self, asm: &mut CodeAssembler, dst: Register, src: AsmRegister64) {
        let dst_off = self.get_reg_offset(dst);
        if dst_off != 0 { // Don't do the write if destination is zero-register
            asm.mov(ptr(r13+dst_off), src).unwrap();
        }
    }
    */

    /// Get the value stored in a register
    pub fn get_reg(&self, reg: Register) -> usize {
        assert!((reg as usize) < 33);
        if reg == Register::Zero { return 0; }
        self.state.regs[reg as usize]
    }

    /// Load a segment from the elf binary into the emulator memory
    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        self.memory.load_segment(segment, data)
    }

    /// Allocate a region of memory in the emulator
    pub fn allocate(&mut self, size: usize, perms: u8) -> Option<usize> {
        self.memory.allocate(size, perms)
    }

    /// Free a previously allocated memory region
    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        self.memory.free(addr)
    }

    /// Print out memory mapped registers
    pub fn dump_regs(&self) {
        println!("ZE  {:#018X}  ra  {:#018X}  sp  {:#018X}  gp  {:#018X}", 
                 self.get_reg(Register::Zero),
                 self.get_reg(Register::Ra),
                 self.get_reg(Register::Sp),
                 self.get_reg(Register::Gp));

        println!("tp  {:#018X}  t0  {:#018X}  t1  {:#018X}  t2  {:#018X}",
                 self.get_reg(Register::Tp),
                 self.get_reg(Register::T0),
                 self.get_reg(Register::T1),
                 self.get_reg(Register::T2));

        println!("s0  {:#018X}  s1  {:#018X}  a0  {:#018X}  a1  {:#018X}",
                 self.get_reg(Register::S0),
                 self.get_reg(Register::S1),
                 self.get_reg(Register::A0),
                 self.get_reg(Register::A1));

        println!("a2  {:#018X}  a3  {:#018X}  a4  {:#018X}  a5  {:#018X}",
                 self.get_reg(Register::A2),
                 self.get_reg(Register::A3),
                 self.get_reg(Register::A4),
                 self.get_reg(Register::A5));

        println!("a6  {:#018X}  a7  {:#018X}  s2  {:#018X}  s3  {:#018X}",
                 self.get_reg(Register::A6),
                 self.get_reg(Register::A7),
                 self.get_reg(Register::S2),
                 self.get_reg(Register::S3));

        println!("s4  {:#018X}  s5  {:#018X}  s6  {:#018X}  s7  {:#018X}",
                 self.get_reg(Register::S4),
                 self.get_reg(Register::S5),
                 self.get_reg(Register::S6),
                 self.get_reg(Register::S7));

        println!("s8  {:#018X}  s9  {:#018X}  s10 {:#018X}  s11 {:#018X}",
                 self.get_reg(Register::S8),
                 self.get_reg(Register::S9),
                 self.get_reg(Register::S10),
                 self.get_reg(Register::S11));

        println!("t3  {:#018X}  t4  {:#018X}  t5  {:#018X}  t6  {:#018X}",
                 self.get_reg(Register::T3),
                 self.get_reg(Register::T4),
                 self.get_reg(Register::T5),
                 self.get_reg(Register::T6));
    }

    /// Runs the jit until exit/crash. It checks if the code at `pc` has already been compiled. If
    /// not it starts by initiating the procedure to compile the code. At this point it has the
    /// jitcache address of where `pc` is jit compiled too. Next it sets up various arguments and
    /// calls this code. These arguments point to structures in memory that the jit-code needs to
    /// convert original addresses to their corresponding jit addresses.
    /// Once the jit exits it collects the reentry_pc (where to continue execution), and the exit
    /// code. It performs an appropriate operation based on the exit code and then continues with
    /// the loop to reenter the jit.
    pub fn run_jit(&mut self) -> Option<Fault> {
        loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Some(Fault::ExecFault(pc)); }

            let jit_addr = match (*self.jit).lookup(pc) {
                Option::None => {
                    // IR instructions + labels at start of each control block
                    let irgraph = self.lift_func(pc).unwrap();

                    // Compile the previously lifted function
                    self.jit.compile(&irgraph, &self.hooks).unwrap()
                },
                Some(addr) => addr
            };

            let exit_code:  usize;
            let reentry_pc: usize;
            unsafe {
                let func = *(&jit_addr as *const usize as *const fn());

                asm!(r#"
                    call {call_dest}
                "#,
                call_dest = in(reg) func,
                out("rax") exit_code,
                out("rcx") reentry_pc,
                in("r12") self.memory.permissions.as_ptr() as u64,
                in("r13") self.memory.memory.as_ptr() as u64,
                in("r14") self.state.regs.as_ptr() as u64,
                in("r15") self.jit.lookup_arr.read().unwrap().as_ptr() as u64,
                );
            }

            self.set_reg(Register::Pc, reentry_pc);

            //if reentry_pc == 0x0000000000010194 {
            //    let v = self.state.regs[Register::A0 as usize];
            //    println!("A0 is: {:#0X}", v);
            //    for i in 0..16 {
            //        println!("Mem is: {:?}", self.memory.memory[v+i as usize]);
            //    }
            //    panic!("");
            //}

            match exit_code {
                1 => { /* Nothing special, just need to compile next code block */ },
                2 => { /* SYSCALL */
                    match self.get_reg(Register::A7) {
                        57 => {
                            syscalls::close(self);
                        },
                        64 => {
                            syscalls::write(self);
                        },
                        80 => {
                            syscalls::fstat(self);
                        },
                        93 => {
                            return syscalls::exit();
                        },
                        214 => {
                            syscalls::brk(self);
                        },
                        _ => { panic!("Unimplemented syscall: {}", self.get_reg(Register::A7)); }
                    }
                },
                3 => { /* Hooked function */
                    if let Some(callback) = self.hooks.get(&reentry_pc) {
                        callback(self).unwrap();
                    } else {
                        error_exit("Attempted to hook invalid function");
                    }
                },
                4 => { /* Debug function, huge performance cost, only use while debugging */
                    self.debug_jit(reentry_pc);
                }
                8 => { /* Attempted to read memory without read permissions */
                    panic!("Invalid memory read: {:#0X}", reentry_pc);
                },
                9 => { /* Attempted to write to memory without write permissions */
                    panic!("Invalid memory write: {:#0X}", reentry_pc);
                },
                _ => panic!("Invalid JIT return code: {:x}", exit_code),
            }
        }
    }

    fn debug_jit(&mut self, pc: usize) {
        let opcode: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
            Fault::ExecFault(pc)).unwrap();
        let instr = decode_instr(opcode);

        println!("\n{:#X}  {:?}", pc, instr);
        self.dump_regs();
    }

    /// Returns a BTreeMap of pc value's at which a label should be created
    fn extract_labels(&self, mut pc: usize, instrs: &[Instr]) -> BTreeMap<usize, u8> {
        let mut ret = BTreeMap::new();

        for instr in instrs {
            match instr {
                Instr::Jal { rd: _, imm} => {
                    ret.insert((pc as i32 + imm) as usize, 0);
                },
                Instr::Beq  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bne  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Blt  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bge  { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bltu { rs1: _, rs2: _, imm, mode: _ } |
                Instr::Bgeu { rs1: _, rs2: _, imm, mode: _ } => {
                    ret.insert((pc as i32 + imm) as usize, 0);
                    ret.insert((pc as i32 + 4) as usize, 0);
                },
                _ => {},
            }
            pc += 4;
        }
        ret
    }

    /// Lift a function into an intermediate representation using the lift helper function
    fn lift_func(&mut self, mut pc: usize) -> Result<IRGraph, ()> {
        let mut irgraph = IRGraph::default();
        let mut instrs: Vec<Instr> = Vec::new();

        let start_pc = pc;

        //println!("PC IS: {:#0X}", pc);
        let end_pc   = start_pc + self.functions.get(&pc).unwrap();

        //println!("({:#0x} : {:#0x})", start_pc, end_pc);

        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);
            instrs.push(instr);
            pc +=4;
        }

        // These are used to determine jump locations ahead of time
        let mut keys = self.extract_labels(start_pc, &instrs);
        keys.insert(start_pc, 0);

        self.lift(&mut irgraph, &instrs, &mut keys, start_pc);

        Ok(irgraph)
    }

    /// This function takes a set of instructions and lifts them into the intermediate
    /// representation. It uses the keys to insert labels where appropriate. These act as start
    /// markers for new code blocks.
    fn lift(&mut self, irgraph: &mut IRGraph, instrs: &[Instr], keys: &mut BTreeMap<usize, u8>,
            mut pc: usize) {

        // Lift instructions until we reach the end of the function
        for instr in instrs {

            irgraph.init_instr(pc);

            if keys.get(&pc).is_some() {
                irgraph.set_label(pc);
            }

            match *instr {
                Instr::Lui {rd, imm} => {
                    irgraph.movi(rd, imm, Flag::Signed);
                },
                Instr::Auipc {rd, imm} => {
                    let sign_extended = (imm as i64 as u64).wrapping_add(pc as u64);
                    irgraph.movi(rd, sign_extended as i32, Flag::Signed);
                },
                Instr::Jal {rd, imm} => {
                    let jmp_target = ((pc as i32) + imm) as usize;

                    if rd != Register::Zero {
                        irgraph.movi(rd, (pc + 4) as i32, Flag::Unsigned);
                    }
                    irgraph.jmp(jmp_target);
                },
                Instr::Jalr {rd, imm, rs1} => {
                    if rd != Register::Zero {
                        irgraph.movi(rd, (pc + 4) as i32, Flag::Signed);
                    }
                    irgraph.jmp_offset(rs1, imm);
                },
                Instr::Beq  { rs1, rs2, imm, mode } |
                Instr::Bne  { rs1, rs2, imm, mode } |
                Instr::Blt  { rs1, rs2, imm, mode } |
                Instr::Bge  { rs1, rs2, imm, mode } |
                Instr::Bltu { rs1, rs2, imm, mode } |
                Instr::Bgeu { rs1, rs2, imm, mode } => {
                    let true_part  = ((pc as i32).wrapping_add(imm)) as usize;
                    let false_part = ((pc as i32).wrapping_add(4)) as usize;

                    match mode {
                        0b000 => { /* BEQ */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                Flag::Equal | Flag::Signed)
                        },
                        0b001 => { /* BNE */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::NEqual | Flag::Signed)
                        },
                        0b100 => { /* BLT */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Less | Flag::Signed)
                        },
                        0b101 => { /* BGE */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Greater | Flag::Signed | Flag::Equal)
                        },
                        0b110 => { /* BLTU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Less | Flag::Unsigned)
                        },
                        0b111 => { /* BGEU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Greater | Flag::Unsigned | Flag::Equal)
                        },
                        _ => { unreachable!(); },
                    }
                },
                Instr::Lb  {rd, rs1, imm, mode} |
                Instr::Lh  {rd, rs1, imm, mode} |
                Instr::Lw  {rd, rs1, imm, mode} |
                Instr::Lbu {rd, rs1, imm, mode} |
                Instr::Lhu {rd, rs1, imm, mode} |
                Instr::Lwu {rd, rs1, imm, mode} |
                Instr::Ld  {rd, rs1, imm, mode} => {
                    match mode {
                        0b000 => irgraph.load(rd, rs1, imm, Flag::Byte | Flag::Signed),    // LB
                        0b001 => irgraph.load(rd, rs1, imm, Flag::Word | Flag::Signed),    // LH
                        0b010 => irgraph.load(rd, rs1, imm, Flag::DWord | Flag::Signed),   // LW
                        0b100 => irgraph.load(rd, rs1, imm, Flag::Byte | Flag::Unsigned),  // LBU
                        0b101 => irgraph.load(rd, rs1, imm, Flag::Word | Flag::Unsigned),  // LHU
                        0b110 => irgraph.load(rd, rs1, imm, Flag::DWord | Flag::Unsigned), // LWU
                        0b011 => irgraph.load(rd, rs1, imm, Flag::QWord),                  // LD
                        _ => unreachable!(),
                    };
                },
                Instr::Sb  {rs1, rs2, imm, mode} |
                Instr::Sh  {rs1, rs2, imm, mode} |
                Instr::Sw  {rs1, rs2, imm, mode} |
                Instr::Sd  {rs1, rs2, imm, mode} => {
                    match mode {
                        0b000 => { irgraph.store(rs1, rs2, imm, Flag::Byte) },  // SB
                        0b001 => { irgraph.store(rs1, rs2, imm, Flag::Word) },  // SH
                        0b010 => { irgraph.store(rs1, rs2, imm, Flag::DWord) }, // SW
                        0b011 => { irgraph.store(rs1, rs2, imm, Flag::QWord) }, // SD
                        _ => { unreachable!(); },
                    }
                },
                Instr::Addi  {rd, rs1, imm } |
                Instr::Slti  {rd, rs1, imm } |
                Instr::Sltiu {rd, rs1, imm } |
                Instr::Xori  {rd, rs1, imm } |
                Instr::Ori   {rd, rs1, imm } |
                Instr::Andi  {rd, rs1, imm } |
                Instr::Slli  {rd, rs1, imm } |
                Instr::Srli  {rd, rs1, imm } |
                Instr::Srai  {rd, rs1, imm } |
                Instr::Addiw {rd, rs1, imm } |
                Instr::Slliw {rd, rs1, imm } |
                Instr::Srliw {rd, rs1, imm } |
                Instr::Sraiw {rd, rs1, imm } => {
                    match instr {
                        Instr::Sltiu { .. } => irgraph.slti(rd, rs1, imm, Flag::Unsigned),
                        Instr::Slti  { .. } => irgraph.slti(rd, rs1, imm, Flag::Signed),
                        Instr::Addi  { .. } => irgraph.addi(rd, rs1, imm, Flag::QWord),
                        Instr::Slli  { .. } => irgraph.shli(rd, rs1, imm, Flag::QWord),
                        Instr::Srli  { .. } => irgraph.shri(rd, rs1, imm, Flag::QWord),
                        Instr::Srai  { .. } => irgraph.sari(rd, rs1, imm, Flag::QWord),
                        Instr::Addiw { .. } => irgraph.addi(rd, rs1, imm, Flag::DWord),
                        Instr::Slliw { .. } => irgraph.shli(rd, rs1, imm, Flag::DWord),
                        Instr::Srliw { .. } => irgraph.shri(rd, rs1, imm, Flag::DWord),
                        Instr::Sraiw { .. } => irgraph.sari(rd, rs1, imm, Flag::DWord),
                        Instr::Xori  { .. } => irgraph.xori(rd, rs1, imm),
                        Instr::Andi  { .. } => irgraph.andi(rd, rs1, imm),
                        Instr::Ori   { .. } => irgraph.ori(rd, rs1, imm),
                        _ => unreachable!(),
                    };
                },
                Instr::Add  {rd, rs1, rs2 } |
                Instr::Sub  {rd, rs1, rs2 } |
                Instr::Sll  {rd, rs1, rs2 } |
                Instr::Slt  {rd, rs1, rs2 } |
                Instr::Sltu {rd, rs1, rs2 } |
                Instr::Xor  {rd, rs1, rs2 } |
                Instr::Srl  {rd, rs1, rs2 } |
                Instr::Sra  {rd, rs1, rs2 } |
                Instr::Or   {rd, rs1, rs2 } |
                Instr::And  {rd, rs1, rs2 } |
                Instr::Addw {rd, rs1, rs2 } |
                Instr::Subw {rd, rs1, rs2 } |
                Instr::Sllw {rd, rs1, rs2 } |
                Instr::Srlw {rd, rs1, rs2 } |
                Instr::Sraw {rd, rs1, rs2 } => {
                    match instr   {
                        Instr::Add  { .. } => irgraph.add(rd, rs1, rs2, Flag::QWord),
                        Instr::Sub  { .. } => irgraph.sub(rd, rs1, rs2, Flag::QWord),
                        Instr::Sll  { .. } => irgraph.shl(rd, rs1, rs2, Flag::QWord),
                        Instr::Slt  { .. } => irgraph.slt(rd, rs1, rs2, Flag::Signed),
                        Instr::Sltu { .. } => irgraph.slt(rd, rs1, rs2, Flag::Unsigned),
                        Instr::Xor  { .. } => irgraph.xor(rd, rs1, rs2),
                        Instr::Srl  { .. } => irgraph.shr(rd, rs1, rs2, Flag::QWord),
                        Instr::Sra  { .. } => irgraph.sar(rd, rs1, rs2, Flag::QWord),
                        Instr::Or   { .. } => irgraph.or(rd, rs1, rs2),
                        Instr::And  { .. } => irgraph.and(rd, rs1, rs2),
                        Instr::Addw { .. } => irgraph.add(rd, rs1, rs2, Flag::DWord),
                        Instr::Subw { .. } => irgraph.sub(rd, rs1, rs2, Flag::DWord),
                        Instr::Sllw { .. } => irgraph.shl(rd, rs1, rs2, Flag::DWord),
                        Instr::Srlw { .. } => irgraph.shr(rd, rs1, rs2, Flag::DWord),
                        Instr::Sraw { .. } => irgraph.sar(rd, rs1, rs2, Flag::DWord),
                        _ => unreachable!(),
                    };
                },
                Instr::Ecall {} => {
                    irgraph.syscall();
                },
                _ => { panic!("A problem occured while lifting to IR: {:?}", instr); },
            }
            pc += 4;
        }
    }
}

/*
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
*/
