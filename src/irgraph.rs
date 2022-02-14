use crate::{
    emulator::Register as PReg,
    ssa_builder::Block,
};

use std::fmt::{self, Formatter, UpperHex};
use num_traits::Signed;
use rustc_hash::FxHashMap;

/// Small helper type that is used to print out hex value eg. -0x20 instead of 0xffffffe0
struct ReallySigned<T: PartialOrd + Signed + UpperHex>(T);
impl<T: PartialOrd + Signed + UpperHex> UpperHex for ReallySigned<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        let bare_hex = format!("{:X}", self.0.abs());
        f.pad_integral(self.0 >= T::zero(), prefix, &bare_hex)
    }
}

/// Register-type used internally by the IR (Register, SSA number for register)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Reg(pub PReg, pub u16);

impl Reg {
    /// Retrivers the set of blocks that a certain register uses
    pub fn get_blocks(self, blocks: &[Block], instrs: &[Instruction]) -> Vec<usize> {
        let mut use_blocks: Vec<usize> = Vec::new();
        for block in blocks {
            if block.instrs(instrs)
                .iter()
                .flat_map(|e| &e.i_reg)
                .any(|e| *e == self) {
                    use_blocks.push(block.index);
                }
        }
        use_blocks
    }
}

impl fmt::Display for Reg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}({})", self.0, self.1)
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Operation {
    Undefined,
    Loadi(i32),
    Jmp(usize),
    Call(usize),
    Branch(usize, usize),
    Syscall,
    JmpReg,
    Ret,
    Phi,
    CallReg,
    Store,
    Load,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Slt,
}

impl Default for Operation {
    fn default() -> Self { Operation::Undefined }
}

/// These are used to give instructions extra information such as signed/unsigned or the type of
/// comparison for branch instructions.
#[derive(Debug, Clone, Copy)]
pub struct Flag;
#[allow(non_upper_case_globals)]
impl Flag {
    pub const NoFlag:   u16 = 0x0;
    pub const Signed:   u16 = 0x1;
    pub const Unsigned: u16 = 0x2;
    pub const Equal:    u16 = 0x4;
    pub const NEqual:   u16 = 0x8;
    pub const Less:     u16 = 0x10;
    pub const Greater:  u16 = 0x20;
    pub const Byte:     u16 = 0x40;
    pub const Word:     u16 = 0x80;
    pub const DWord:    u16 = 0x100;
    pub const QWord:    u16 = 0x200;
}

/// The instructions used in the IR. Layed out in a way that is efficient memory wise and lets us
/// easily determine if the instruction has input/output fields.
#[derive(Debug, Clone, Default)]
pub struct Instruction {
    pub op:     Operation,
    pub i_reg:  Vec<Reg>,
    pub o_reg:  Option<Reg>,
    pub flags:  u16,
    pub pc:     Option<usize>,
}

impl Instruction {
    pub fn is_phi_function(&self) -> bool {
        self.op == Operation::Phi
    }
}

/// Pretty printing for the instructions
impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.op {
            Operation::Loadi(x) => {
                write!(f, "{:#08X}  {} = {:#0X}",
                       self.pc.unwrap_or(0), self.o_reg.unwrap(), ReallySigned(x as i32))
            },
            Operation::Jmp(x) => {
                write!(f, "{:#08X}  Jmp {:#0x?}", self.pc.unwrap_or(0), x)
            },
            Operation::Call(x) => {
                write!(f, "{:#08X}  Call {:#0x?}", self.pc.unwrap_or(0), x)
            },
            Operation::CallReg => {
                write!(f, "{:#08X}  Call {}", self.pc.unwrap_or(0), self.i_reg[0])
            },
            Operation::Branch(x, y) => {
                match self.flags & 0b111100 {
                    0b000100 => {
                        write!(f, "{:#08X}  if {} == {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    0b001000 => {
                        write!(f, "{:#08X}  if {} != {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    0b010000 => {
                        write!(f, "{:#08X}  if {} < {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    0b100000 => {
                        write!(f, "{:#08X}  if {} > {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    _ => { unreachable!() },
                }
            },
            Operation::Phi => {
                write!(f, "{:#08X}  {} = φ({}, {})", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            }
            Operation::Syscall => {
                write!(f, "{:#08X}  Syscall", self.pc.unwrap_or(0))
            },
            Operation::JmpReg => {
                write!(f, "{:#08X}  Jmp {}", self.pc.unwrap_or(0), self.i_reg[0])
            },
            Operation::Ret => {
                write!(f, "{:#08X}  Ret", self.pc.unwrap_or(0))
            },
            Operation::Store => {
                write!(f, "{:#08X}  [{}] = {}", self.pc.unwrap_or(0), self.i_reg[1], self.i_reg[0])
            },
            Operation::Load => {
                write!(f, "{:#08X}  {} = [{}]", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                    self.i_reg[0])
            },
            Operation::Add => {
                write!(f, "{:#08X}  {} = {} + {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Sub => {
                write!(f, "{:#08X}  {} = {} - {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::And => {
                write!(f, "{:#08X}  {} = {} & {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Or => {
                write!(f, "{:#08X}  {} = {} | {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Xor => {
                write!(f, "{:#08X}  {} = {} ^ {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Shl => {
                write!(f, "{:#08X}  {} = {} << {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Shr => {
                write!(f, "{:#08X}  {} = {} >> {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Sar => {
                write!(f, "{:#08X}  {} = {} >> {} [A]", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Slt => {
                write!(f, "{:#08X}  {} = {} < {} ? 1 : 0", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg[0], self.i_reg[1])
            },
            _ => { unreachable!() },
        }
    }
}

/// Basic wrapper around instructions that keeps track of cur_pc.
#[derive(Debug)]
pub struct IRGraph {
    /// List of all instructions
    pub instrs: Vec<Instruction>,

    /// Labels indicating controlflow (instrs_index, pc)
    pub labels: FxHashMap<usize, usize>,

    /// Since multiple IR instructions can be mapped to a single original instruction, this is used
    /// to only assign the pc to the first IR-instruction is generated for an original instruction.
    cur_pc: Option<usize>,
}

impl Default for IRGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl IRGraph {
    pub fn new() -> Self {
        IRGraph {
            instrs: Vec::new(),
            labels: FxHashMap::default(),
            cur_pc: None,
        }
    }

    /// Initialize the cur_pc variable which is used to set the pc value in the IR instructions
    pub fn init_instr(&mut self, pc: usize) {
        self.cur_pc = Some(pc);
    }

    /// Get an IRReg for a physical register
    pub fn get_reg(&self, phys_reg: PReg) -> Reg {
        Reg(phys_reg, 0)
    }

    /// Insert a label into the irgraph using the current pc
    pub fn set_label(&mut self, pc: usize) {
        self.labels.insert(pc, self.instrs.len());
    }

    /// r1 = #imm
    pub fn loadi(&mut self, r1: PReg, imm: i32, flag: u16) -> PReg {
        let v1 = self.get_reg(r1);
        self.instrs.push( Instruction {
            op: Operation::Loadi(imm),
            i_reg: Vec::new(),
            o_reg: Some(v1),
            flags: flag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// Jmp addr
    pub fn jmp(&mut self, addr: usize) {
        self.instrs.push( Instruction {
            op: Operation::Jmp(addr),
            i_reg: Vec::new(),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Call target
    pub fn call(&mut self, addr: usize) {
        self.instrs.push( Instruction {
            op: Operation::Call(addr),
            i_reg: Vec::new(),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Return
    pub fn ret(&mut self) {
        self.instrs.push( Instruction {
            op: Operation::Ret,
            i_reg: Vec::new(),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Jmp r1
    pub fn jmp_reg(&mut self, r1: PReg) {
        let v1 = self.get_reg(r1);
        self.instrs.push( Instruction {
            op: Operation::JmpReg,
            i_reg: vec![v1],
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Call r1
    pub fn call_reg(&mut self, r1: PReg) {
        let v1 = self.get_reg(r1);
        self.instrs.push( Instruction {
            op: Operation::CallReg,
            i_reg: vec![v1],
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Branch to either false_part or true_part, flags determine what kind of compare instruction
    /// is supposed to be inserted
    pub fn branch(&mut self, r2: PReg, r3: PReg, true_part: usize, false_part: usize, flags: u16) {
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Branch(true_part, false_part),
            i_reg: vec![v2, v3],
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// r1 = [r2]
    pub fn load(&mut self, r1: PReg, r2: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        self.instrs.push( Instruction {
            op: Operation::Load,
            i_reg: vec![v2],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// [r3] = r2
    pub fn store(&mut self, r2: PReg, r3: PReg, flags: u16) {
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Store,
            i_reg: vec![v2, v3],
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Set res_reg if rs1_reg is less than imm_reg
    pub fn slt(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Slt,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 + r3
    pub fn add(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Add,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 - r3
    pub fn sub(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Sub,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 ^ r3
    pub fn xor(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Xor,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 | r3
    pub fn or(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Or,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 & r3
    pub fn and(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::And,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 << r3
    pub fn shl(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Shl,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> r3 (Logical)
    pub fn shr(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Shr,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> r3 (Arithmetic)
    pub fn sar(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        let v1 = self.get_reg(r1);
        let v2 = self.get_reg(r2);
        let v3 = self.get_reg(r3);
        self.instrs.push( Instruction {
            op: Operation::Sar,
            i_reg: vec![v2, v3],
            o_reg: Some(v1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// Syscall instruction
    pub fn syscall(&mut self) {
         self.instrs.push( Instruction {
            op: Operation::Syscall,
            i_reg: Vec::new(),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }
}
