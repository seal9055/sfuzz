use std::fs::File;
use std::io::Write;
use std::fmt::{self, Formatter, UpperHex};
use num_traits::Signed;

use rustc_hash::FxHashMap;
use petgraph::Graph;
use petgraph::dot::{Dot, Config};

/// Errors that can occur during IR Operations
#[derive(Debug)]
pub enum Error {
    /// Ran out of registers for graph
    OutOfRegs,

    /// Ran out of labels for graph
    OutOfLabels,
}

/// Small helper type that is used to print out hex value eg. -0x20 instead of 0xffffffe0
struct ReallySigned<T: PartialOrd + Signed + UpperHex>(T);
impl<T: PartialOrd + Signed + UpperHex> UpperHex for ReallySigned<T> {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        let prefix = if f.alternate() { "0x" } else { "" };
        let bare_hex = format!("{:X}", self.0.abs());
        f.pad_integral(self.0 >= T::zero(), prefix, &bare_hex)
    }
}

/// Register-type used internally by the IR
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reg(pub u16);

/// A label used for control flow in the graph
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Label(pub u16);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Operation {
    IDK,
    Loadi(i32),
    Jmp(usize),
    Call(usize),
    Branch(usize, usize),
    Label(usize),
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
    fn default() -> Self { Operation::IDK }
}

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

#[derive(Debug, Copy, Clone, Default)]
pub struct Instruction {
    pub op:     Operation,
    pub i_reg:  (Option<Reg>, Option<Reg>),
    pub o_reg:  Option<Reg>,
    pub flags:  u16,
    pub pc:     Option<usize>,
}

/// Pretty printing for the instructions
impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.op {
            Operation::Loadi(x) => {
                write!(f, "{:#08X}  {:?} = {:#0X}",
                       self.pc.unwrap_or(0), self.o_reg.unwrap(), ReallySigned(x as i32))
            },
            Operation::Jmp(x) => {
                write!(f, "{:#08X}  Jmp {:#0x?}", self.pc.unwrap_or(0), x)
            },
            Operation::Call(x) => {
                write!(f, "{:#08X}  Call {:#0x?}", self.pc.unwrap_or(0), x)
            },
            Operation::CallReg => {
                write!(f, "{:#08X}  Call {:#0x?}", self.pc.unwrap_or(0), self.i_reg.0.unwrap())
            },
            Operation::Branch(x, y) => {
                match self.flags & 0b111100 {
                    0b000100 => {
                        write!(f, "{:#08X}  BE ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0), y, x)
                    },
                    0b001000 => {
                        write!(f, "{:#08X}  BNE ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0), y, x)
                    },
                    0b010000 => {
                        write!(f, "{:#08X}  BLT ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0), y, x)
                    },
                    0b100000 => {
                        write!(f, "{:#08X}  BGT ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0), y, x)
                    },
                    _ => { unreachable!() },
                }
            },
            Operation::Phi => {
                write!(f, "{:#08X}  φ ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                       self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            }
            Operation::Label(x) => {
                write!(f, "\t\tLabel @ {:#0X?}\n", x)
            },
            Operation::Syscall => {
                write!(f, "{:#08X}  Syscall", self.pc.unwrap_or(0))
            },
            Operation::JmpReg => {
                write!(f, "{:#08X}  Jmp {:?}", self.pc.unwrap_or(0), self.i_reg.0.unwrap())
            },
            Operation::Ret => {
                write!(f, "{:#08X}  Ret", self.pc.unwrap_or(0))
            },
            Operation::Store => {
                write!(f, "{:#08X}  [{:?}] = {:?}", self.pc.unwrap_or(0), self.i_reg.1.unwrap(),
                    self.i_reg.0.unwrap())
            },
            Operation::Load => {
                write!(f, "{:#08X}  {:?} = [{:?}]", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                    self.i_reg.0.unwrap())
            },
            Operation::Add => {
                write!(f, "{:#08X}  {:?} = {:?} + {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Sub => {
                write!(f, "{:#08X}  {:?} = {:?} - {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::And => {
                write!(f, "{:#08X}  {:?} = {:?} & {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Or => {
                write!(f, "{:#08X}  {:?} = {:?} | {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Xor => {
                write!(f, "{:#08X}  {:?} = {:?} ^ {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Shl => {
                write!(f, "{:#08X}  {:?} = {:?} << {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Shr => {
                write!(f, "{:#08X}  {:?} = {:?} >> {:?}", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Sar => {
                write!(f, "{:#08X}  {:?} = {:?} >> {:?} [A]", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            Operation::Slt => {
                write!(f, "{:#08X}  {:?} = {:?} < {:?} ? 1 : 0", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg.0.unwrap(), self.i_reg.1.unwrap())
            },
            _ => { unreachable!() },
        }
    }
}

#[derive(Debug)]
pub struct IRGraph {
    /// List of all instructions
    pub instrs: Vec<Instruction>,

    /// Currently available register index
    next_reg: Reg,

    /// This is used to map track the pc of the currently executing instruction
    cur_pc: Option<usize>,

    // Track which functions have already been jitted, and their adresses, prob in the jit though
}

impl Default for IRGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl IRGraph {
    pub fn new() -> Self {
        IRGraph {
            instrs:     Vec::new(),
            next_reg:   Reg(0),
            cur_pc:     None,
        }
    }

    /// Optimize the IRGraph
    pub fn optimize(&mut self) -> Option<()> {
        // Probably convert to cfg notation before starting optimizations
        // Need to research different types of optimizations a little first, the main optimization
        // will be to improve codegen involving immediate instructions because these currently take
        // way too many instructions


        // 1. lift into cfg
        // 2. add functionality to emit a graph

        Some(())
    }

    pub fn dump_instrs_dot(&self) {
        let instrs = self.instrs.clone();
        let mut graph = Graph::<_, i32>::new();
        let mut map: FxHashMap<usize, usize> = FxHashMap::default();
        let mut edges: Vec<(u32, u32)> = Vec::new();

        for (i, instr) in instrs.into_iter().enumerate() {
            match instr.op {
                Operation::Branch(x, _) => {
                    if map.get(&x).is_some() {
                        let v = *map.get(&x).unwrap() as u32;
                        edges.push( (i as u32, v) );
                    }
                    map.insert(x, i);
                    edges.push( (i as u32, (i + 1) as u32) );
                },
                Operation::Label(x) => {
                    if map.get(&x).is_some() {
                        let v = *map.get(&x).unwrap() as u32;
                        edges.push( (v, (i) as u32) );
                    }
                    map.insert(x, i);
                    edges.push( (i as u32, (i + 1) as u32) );
                },
                Operation::Jmp(x) => {
                    if map.get(&x).is_some() {
                        let v = *map.get(&x).unwrap() as u32;
                        edges.push( (i as u32, v) );
                    }
                    map.insert(x, i);
                }
                Operation::Call(x) => {
                    if map.get(&x).is_some() {
                        let v = *map.get(&x).unwrap() as u32;
                        edges.push( (i as u32, v) );
                    }
                    map.insert(x, i);
                    edges.push( (i as u32, (i + 1) as u32) );
                }
                _ => {
                    edges.push( (i as u32, (i + 1) as u32) );
                },
            };
            graph.add_node(instr);
        }
        for edge in edges.iter().take(edges.len() - 1) {
            graph.extend_with_edges([edge]);
        }
        let mut f = File::create("graph.dot").unwrap();
        let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
        f.write_all(output.as_bytes()).expect("could not write file");
    }

    /// Initialize the cur_pc variable which is used to set the pc value in the ir instructions
    pub fn init_instr(&mut self, pc: usize) {
        self.cur_pc = Some(pc);
    }

    /// Allocate new Register for IRGraph
    fn alloc_reg(&mut self) -> Result<Reg, Error> {
        let ret = self.next_reg;
        self.next_reg = Reg(self.next_reg.0.checked_add(1).ok_or(Error::OutOfRegs)?);
        Ok(ret)
    }

    /// Allocate new Label for IRGraph
    pub fn set_label(&mut self, pc: usize) {
        //self.labels.insert(self.cur_pc.unwrap(), self.instrs.len() as u16);
        self.instrs.push( Instruction {
            op: Operation::Label(pc),
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: None,
        });
    }

    /// Load an immediate value into a register
    pub fn loadi(&mut self, imm: i32, flag: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Loadi(imm),
            i_reg: (None, None),
            o_reg: Some(reg),
            flags: flag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn jmp(&mut self, target: usize) {
        self.instrs.push( Instruction {
            op: Operation::Jmp(target),
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn call(&mut self, target: usize) {
        self.instrs.push( Instruction {
            op: Operation::Call(target),
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn ret(&mut self, target: Reg) {
        self.instrs.push( Instruction {
            op: Operation::Ret,
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn jmp_reg(&mut self, target: Reg) {
        self.instrs.push( Instruction {
            op: Operation::JmpReg,
            i_reg: (Some(target), None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn call_reg(&mut self, target: Reg) {
        self.instrs.push( Instruction {
            op: Operation::CallReg,
            i_reg: (Some(target), None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn branch(&mut self, reg1: Reg, reg2: Reg, true_part: usize, false_part: usize, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Branch(true_part, false_part),
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    pub fn load(&mut self, reg1: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Load,
            i_reg: (Some(reg1), None),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn store(&mut self, rs2_reg: Reg, mem_addr: Reg, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Store,
            i_reg: (Some(rs2_reg), Some(mem_addr)),
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Set res_reg if rs1_reg is less than imm_reg
    pub fn slt(&mut self, rs1_reg: Reg, imm_reg: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Slt,
            i_reg: (Some(rs1_reg), Some(imm_reg)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    /// Add 2 registers and store the result in a new register
    pub fn add(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Add,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    /// Subtract ret2 from reg1 and store the result in a new register
    pub fn sub(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Sub,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn xor(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Xor,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn or(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Or,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn and(&mut self, reg1: Reg, reg2: Reg) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::And,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn shl(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Shl,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn shr(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Shr,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn sar(&mut self, reg1: Reg, reg2: Reg, flags: u16) -> Reg {
        let reg = self.alloc_reg().unwrap();
        self.instrs.push( Instruction {
            op: Operation::Sar,
            i_reg: (Some(reg1), Some(reg2)),
            o_reg: Some(reg),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        reg
    }

    pub fn syscall(&mut self) {
         self.instrs.push( Instruction {
            op: Operation::Syscall,
            i_reg: (None, None),
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }
}
