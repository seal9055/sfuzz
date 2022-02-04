use crate::{
    mmu::{Mmu, Perms},
    elfparser,
    riscv::{decode_instr, Instr},
    jit::Jit,
    syscalls,
    error_exit,
    irgraph::{IRGraph, Flag, Reg as IRReg},
    cfg::{CFG},
};

use std::sync::Arc;
use std::collections::{BTreeMap, BTreeSet};
use std::arch::asm;
use rustc_hash::FxHashMap;
use array_tool::vec::{Intersect, Uniq};

pub const STDIN:  isize = 0;
pub const STDOUT: isize = 1;
pub const STDERR: isize = 2;

/// 33 RISCV Registers
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
    Z1, // Z[1-2] are temporary registers used to generate ir instructions
    Z2,
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
#[derive(Clone, Copy, Debug, PartialEq)] pub enum Fault {
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

#[derive(Clone)]
pub struct Emulator {
    pub memory: Mmu,

    pub state: State,

    pub hooks: FxHashMap<usize, fn(&mut Emulator) -> Result<(), Fault>>,

    pub jit: Arc<Jit>,

    pub fd_list: Vec<isize>,

    /// Mapping between function address and function size
    pub functions: FxHashMap<usize, usize>,
}

impl Emulator {
    pub fn new(size: usize, jit: Arc<Jit>) -> Self {
        Emulator {
            memory: Mmu::new(size),
            state: State::default(),
            hooks: FxHashMap::default(),
            jit,
            fd_list: vec![STDIN, STDOUT, STDERR],
            functions: FxHashMap::default(),
        }
    }

    #[must_use]
    pub fn fork(&self) -> Self {
        Emulator {
            memory:     self.memory.fork(),
            state:      State::default(),
            hooks:      self.hooks.clone(),
            jit:        self.jit.clone(),
            fd_list:    self.fd_list.clone(),
            functions:  self.functions.clone(),
        }
    }

    pub fn reset(&mut self, original: &Self) {
        self.memory.reset(&original.memory);
        self.state.regs = original.state.regs;

        self.fd_list.clear();
        self.fd_list.extend_from_slice(&original.fd_list);
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

    pub fn dump_regs(&self) {
        println!("zero {:x?}", self.get_reg(Register::Zero));
        println!("ra   {:x?}", self.get_reg(Register::Ra));
        println!("sp   {:x?}", self.get_reg(Register::Sp));
        println!("gp   {:x?}", self.get_reg(Register::Gp));
        println!("tp   {:x?}", self.get_reg(Register::Tp));
        println!("t0   {:x?}", self.get_reg(Register::T0));
        println!("t1   {:x?}", self.get_reg(Register::T1));
        println!("t2   {:x?}", self.get_reg(Register::T2));
        println!("s0   {:x?}", self.get_reg(Register::S0));
        println!("s1   {:x?}", self.get_reg(Register::S1));
        println!("a0   {:x?}", self.get_reg(Register::A0));
        println!("a1   {:x?}", self.get_reg(Register::A1));
        println!("a2   {:x?}", self.get_reg(Register::A2));
        println!("a3   {:x?}", self.get_reg(Register::A3));
        println!("a4   {:x?}", self.get_reg(Register::A4));
        println!("a5   {:x?}", self.get_reg(Register::A5));
        println!("a6   {:x?}", self.get_reg(Register::A6));
        println!("a7   {:x?}", self.get_reg(Register::A7));
        println!("s2   {:x?}", self.get_reg(Register::S2));
        println!("s3   {:x?}", self.get_reg(Register::S3));
        println!("s4   {:x?}", self.get_reg(Register::S4));
        println!("s5   {:x?}", self.get_reg(Register::S5));
        println!("s6   {:x?}", self.get_reg(Register::S6));
        println!("s7   {:x?}", self.get_reg(Register::S7));
        println!("s8   {:x?}", self.get_reg(Register::S8));
        println!("s9   {:x?}", self.get_reg(Register::S9));
        println!("s10  {:x?}", self.get_reg(Register::S10));
        println!("s11  {:x?}", self.get_reg(Register::S11));
        println!("t3   {:x?}", self.get_reg(Register::T3));
        println!("t4   {:x?}", self.get_reg(Register::T4));
        println!("t5   {:x?}", self.get_reg(Register::T5));
        println!("t6   {:x?}", self.get_reg(Register::T6));
        println!("pc   {:x?}", self.get_reg(Register::Pc));
    }

    pub fn run_jit(&mut self) -> Option<Fault> {
        loop {
            let pc = self.get_reg(Register::Pc);

            // Error out if code was unaligned.
            // since Riscv instructions are always 4-byte aligned this is a bug
            if pc & 3 != 0 { return Some(Fault::ExecFault(pc)); }

            let jit_addr = match (*self.jit).lookup(pc) {
                None => {
                    let mut irgraph = self.lift_func(pc).unwrap();

                    let cfg = CFG::new(&irgraph);
                    cfg.dump_dot();

                    for b in &cfg.blocks {
                        println!("[");
                        for instr in b {
                            println!("{}", instr);
                        }
                        println!("]\n");
                    }
                    println!("edges: {:?}", cfg.edges);

                    irgraph.optimize();
                    (*self.jit).compile(irgraph).unwrap()
                }
                Some(addr) => { addr }
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
                out("rdx") _,
                in("r13") self.state.regs.as_ptr() as u64,
                in("r14") self.memory.memory.as_ptr() as u64,
                in("r15") self.jit.lookup_arr.read().unwrap().as_ptr() as u64,
                );
            }

            self.set_reg(Register::Pc, reentry_pc);

            match exit_code {
                1 => { /* Nothing special, just need to compile next code block */
                },
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
                _ => { unreachable!(); }
            }
        }
    }

    /// Returns a BTreeMap of pc value's at which a label should be created
    fn extract_labels(&self, mut pc: usize, instrs: &Vec<Instr>) -> BTreeMap<usize, u8> {
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

    fn lift_func(&mut self, mut pc: usize) -> Result<IRGraph, ()> {
        let mut irgraph = IRGraph::default();
        let mut instrs: Vec<Instr> = Vec::new();

        pc = 0x100b0;
        let start_pc = pc;
        //let end_pc   = start_pc + self.functions.get(&pc).unwrap();
        let end_pc = 0x1034;

        while pc < end_pc {
            let opcodes: u32 = self.memory.read_at(pc, Perms::READ | Perms::EXECUTE).map_err(|_|
                Fault::ExecFault(pc)).unwrap();
            let instr = decode_instr(opcodes);
            instrs.push(instr);
            pc +=4;
        }

        // These are used to determine jump locations ahead of time
        let mut keys: Vec<usize> = self.extract_labels(start_pc, &instrs).keys().cloned().collect();
        keys.insert(0, start_pc);

        println!("keys: {:x?}", keys);

        // Add a label b4 first instruction of this function
        //irgraph.set_label(start_pc);

        self.lift(&mut irgraph, &instrs, &mut keys, start_pc);

        Ok(irgraph)
    }

    /// Lift a function into an immediate representation that can be used to apply optimizations and
    /// compile into the final jit-code
    fn lift(&mut self, irgraph: &mut IRGraph, instrs: &Vec<Instr>, keys: &mut Vec<usize>,
            mut pc: usize) {

        // Lift instructions until we reach the end of the function
        for instr in instrs {

            irgraph.init_instr(pc);

            if !keys.is_empty() && pc == keys[0] {
                keys.remove(0);
                irgraph.set_label(pc);
            }

            match *instr {
                Instr::Lui {rd, imm} => {
                    irgraph.loadi(rd, imm, Flag::Signed);
                },
                Instr::Auipc {rd, imm} => {
                    let val = (imm).wrapping_add(pc as i32);
                    let res = irgraph.loadi(rd, val, Flag::Signed);
                },
                Instr::Jal {rd, imm} => {
                    let ret_val = pc.wrapping_add(4);
                    let jmp_target = ((pc as i32).wrapping_add(imm)) as usize;

                    // Load return value into newly allocated register
                    if rd != Register::Zero {
                        let res = irgraph.loadi(rd, ret_val as i32, Flag::Unsigned);
                        irgraph.call(jmp_target);
                    } else {
                        irgraph.jmp(jmp_target);
                    }
                },
                Instr::Jalr {rd, imm, rs1} => {
                    let ret_val = pc.wrapping_add(4);

                    let imm_reg = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let jmp_target = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                    if rd != Register::Zero {
                        let res = irgraph.loadi(rd, ret_val as i32, Flag::Unsigned);
                        irgraph.call_reg(jmp_target)
                    } else {
                        irgraph.ret();
                    }
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
                                           Flag::Greater | Flag::Signed)
                        },
                        0b110 => { /* BLTU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Less | Flag::Signed)
                        },
                        0b111 => { /* BGEU */
                            irgraph.branch(rs1, rs2, true_part, false_part,
                                           Flag::Greater | Flag::Signed)
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
                    let imm_reg  = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let mem_addr = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                    let res = match mode {
                        0b000 => irgraph.load(rd, mem_addr, Flag::Byte | Flag::Signed),    // LB
                        0b001 => irgraph.load(rd, mem_addr, Flag::Word | Flag::Signed),    // LH
                        0b010 => irgraph.load(rd, mem_addr, Flag::DWord | Flag::Signed),   // LW
                        0b100 => irgraph.load(rd, mem_addr, Flag::Byte | Flag::Unsigned),  // LBU
                        0b101 => irgraph.load(rd, mem_addr, Flag::Word | Flag::Unsigned),  // LHU
                        0b110 => irgraph.load(rd, mem_addr, Flag::DWord | Flag::Unsigned), // LWU
                        0b011 => irgraph.load(rd, mem_addr, Flag::QWord),                  // LD
                        _ => unreachable!(),
                    };
                },
                Instr::Sb  {rs1, rs2, imm, mode} |
                Instr::Sh  {rs1, rs2, imm, mode} |
                Instr::Sw  {rs1, rs2, imm, mode} |
                Instr::Sd  {rs1, rs2, imm, mode} => {
                    let imm_reg  = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let mem_addr = irgraph.add(Register::Z2, rs1, imm_reg, Flag::DWord);

                    match mode {
                        0b000 => { irgraph.store(rs2, mem_addr, Flag::Byte) },  // SB
                        0b001 => { irgraph.store(rs2, mem_addr, Flag::Word) },  // SH
                        0b010 => { irgraph.store(rs2, mem_addr, Flag::DWord) }, // SW
                        0b011 => { irgraph.store(rs2, mem_addr, Flag::QWord) }, // SD
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
                    let imm_reg = irgraph.loadi(Register::Z1, imm, Flag::Signed);
                    let res = match instr {
                        Instr::Addi  { .. } => irgraph.add(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Slti  { .. } => irgraph.slt(rd, rs1, imm_reg, Flag::Signed),
                        Instr::Sltiu { .. } => irgraph.slt(rd, rs1, imm_reg, Flag::Unsigned),
                        Instr::Xori  { .. } => irgraph.xor(rd, rs1, imm_reg),
                        Instr::Ori   { .. } => irgraph.or(rd, rs1, imm_reg),
                        Instr::Andi  { .. } => irgraph.and(rd, rs1, imm_reg),
                        Instr::Slli  { .. } => irgraph.shl(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Srli  { .. } => irgraph.shr(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Srai  { .. } => irgraph.sar(rd, rs1, imm_reg, Flag::QWord),
                        Instr::Addiw { .. } => irgraph.add(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Slliw { .. } => irgraph.shl(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Srliw { .. } => irgraph.shr(rd, rs1, imm_reg, Flag::DWord),
                        Instr::Sraiw { .. } => irgraph.sar(rd, rs1, imm_reg, Flag::DWord),
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
                    let res =  match instr   {
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
                _ => { panic!("a problem in lift"); },
            }
            pc += 4;
        }

        //for instr in &irgraph.instrs {
        //    println!("{:x?}", instr);
        //}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary() {
        let mut instrs: Vec<Instr> = Vec::new();
        let jit = Arc::new(Jit::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(64 * 1024 * 1024, jit);

        /*0x1000*/ instrs.push(Instr::Lui { rd: Register::A0, imm: 20 });
        /*0x1004*/ instrs.push(Instr::Lui { rd: Register::A1, imm: 10 });
        /*0x1008*/ instrs.push(Instr::Beq { rs1: Register::A0, rs2: Register::A1,
            imm: 0x20, mode: 0});

        //b1
        /*0x100c*/ instrs.push(Instr::Add { rd: Register::A2, rs1: Register::A0,
            rs2: Register::A1 });
        /*0x1010*/ instrs.push(Instr::Lui { rd: Register::A3, imm: 1 });
        /*0x1014*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: 0x4 }); //goto b2

        //b2
        /*0x1018*/ instrs.push(Instr::Addi { rd: Register::A4, rs1: Register::A2, imm: 5});
        /*0x101c*/ instrs.push(Instr::Addi { rd: Register::A5, rs1: Register::A4, imm: 1});
        /*0x1020*/ instrs.push(Instr::Addi { rd: Register::A6, rs1: Register::A3, imm: 0 });
        /*0x1024*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: 0x10 }); //goto end

        //b3
        /*0x1028*/ instrs.push(Instr::Sub { rd: Register::A2, rs1: Register::A0,
            rs2: Register::A1 });
        /*0x102c*/ instrs.push(Instr::Lui { rd: Register::A3, imm: 2 });
        /*0x1030*/ instrs.push(Instr::Jal { rd: Register::Zero, imm: -0x18 }); //goto b2

        /*0x1034*/ instrs.push(Instr::Lui { rd: Register::Zero, imm: 0 });

        // end

        let mut keys = vec![0x1000, 0x100c, 0x1018, 0x1028, 0x1034];
        let mut irgraph = IRGraph::default();
        emu.lift(&mut irgraph, &instrs, &mut keys, 0x1000);

        let mut cfg = CFG::new(&irgraph);
        cfg.dump_dot();

        cfg.instrs = irgraph.instrs;
        cfg.convert_to_ssa();

        panic!("done");

        //let df_list = emu.find_domfrontier(&mut dom_tree, &cfg, &mut dom_set);
        //let (var_list, var_origin, varlist_origin, var_tuple) =
        //    emu.find_var_origin(&instrs, &mut leader_set);
        //let (def_sites, var_phi, phi_func) = emu.insert_phi_func(&cfg, &instrs, &df_list,
        //                                                            &varlist_origin, &var_tuple);

        //emu.rename_regs(&cfg, &mut instrs, &mut leader_set, &dom_tree,
        //                &var_list, &var_origin, &phi_func);

        //println!("cfg: {:?}", cfg);
        //println!("leader_set: {:?}", leader_set);
        //println!("dom_tree: {:?}", dom_tree);
        //println!("dom_set: {:?}", dom_set);
        //println!("df_list: {:?}", df_list);
        //println!("var_list: {:?}", var_list);
        //println!("var_list_origin: {:?}", varlist_origin);
        //println!("var_tuple: {:?}", var_tuple);
        //println!("def_sites: {:?}\n", def_sites);
        //println!("var_phi: {:?}\n", var_phi);
        //println!("phi_func: {:?}\n", phi_func);
    }

    #[test]
    fn temporary2() {
        let jit = Arc::new(Jit::new(16 * 1024 * 1024));
        let mut emu = Emulator::new(64 * 1024 * 1024, jit);

        let irgraph = emu.lift_func(0x100b0);

        //for instr in irgraph {
            //println!("{:?}", instr);
        //}
    }
}
