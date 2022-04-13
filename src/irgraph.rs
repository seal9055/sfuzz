use crate::{
    emulator::Register as PReg,
    irgraph::Val::{Reg, Imm},
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

/// Value used to specify both inputs and outputs for intermediate representation
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Val {
    Reg(PReg),
    Imm(i32),
}

impl fmt::Display for Val {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Reg(v) => {
                write!(f, "{:?}", v)
            },
            Imm(v) => {
                write!(f, "{}", v)
            },
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Operation {
    Undefined,
    Jmp(usize),
    JmpOff(i32),
    Branch(usize, usize),
    Syscall,
    Store,
    Load,
    Mov,
    Add,
    Sub,
    And,
    Or,
    Xor,
    Shl,
    Shr,
    Sar,
    Slt,
    Nop,
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
    pub op:    Operation,
    pub i_reg: Vec<Val>,
    pub o_reg: Option<PReg>,
    pub flags: u16,
    pub pc:    Option<usize>,
}

impl Instruction {
    pub fn is_jump(&self) -> bool {
        matches!(self.op, Operation::Jmp(_) | Operation::Branch(..))
    }
}

/// Pretty printing for the instructions
impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.op {
            Operation::Jmp(x) => {
                write!(f, "{:#08X}  Jmp {:#0x?}", self.pc.unwrap_or(0), x)
            },
            Operation::JmpOff(x) => {
                write!(f, "{:#08X}  Jmp ({:?} + {:#X})", self.pc.unwrap_or(0), self.i_reg[0], 
                       ReallySigned(x as i32))
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
                    0b100100 => {
                        write!(f, "{:#08X}  if {} >= {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    0b010100 => {
                        write!(f, "{:#08X}  if {} <= {} ({:#0X?}, {:#0X?})", self.pc.unwrap_or(0),
                               self.i_reg[0], self.i_reg[1], y, x)
                    },
                    _ => { panic!("branch with flag: {}", self.flags & 0b111100); },
                }
            },
            Operation::Syscall => {
                write!(f, "{:#08X}  Syscall", self.pc.unwrap_or(0))
            },
            Operation::Store => {
                write!(f, "{:#08X}  [{}+{}] = {}", self.pc.unwrap_or(0), self.i_reg[0], 
                       self.i_reg[2], self.i_reg[1])
            },
            Operation::Load => {
                write!(f, "{:#08X}  {:?} = [{}+{}]", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                    self.i_reg[0], self.i_reg[1])
            },
            Operation::Add => {
                write!(f, "{:#08X}  {:?} = {} + {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Sub => {
                write!(f, "{:#08X}  {:?} = {} - {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::And => {
                write!(f, "{:#08X}  {:?} = {} & {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Or => {
                write!(f, "{:#08X}  {:?} = {} | {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Xor => {
                write!(f, "{:#08X}  {:?} = {} ^ {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Shl => {
                write!(f, "{:#08X}  {:?} = {} << {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Shr => {
                write!(f, "{:#08X}  {:?} = {} >> {}", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Sar => {
                write!(f, "{:#08X}  {:?} = {} >> {} [A]", self.pc.unwrap_or(0), self.o_reg.unwrap(),
                       self.i_reg[0], self.i_reg[1])
            },
            Operation::Slt => {
                write!(f, "{:#08X}  {:?} = {} < {} ? 1 : 0", self.pc.unwrap_or(0),
                       self.o_reg.unwrap(), self.i_reg[0], self.i_reg[1])
            },
            Operation::Mov => {
                write!(f, "{:#08X}  {:?} = {}", self.pc.unwrap_or(0), 
                       self.o_reg.unwrap(), self.i_reg[0])
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

    /// Insert a label into the irgraph using the current pc
    pub fn set_label(&mut self, pc: usize) {
        self.labels.insert(pc, self.instrs.len());
    }

    /// r1 = imm
    pub fn movi(&mut self, r1: PReg, imm: i32, flag: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Mov,
            i_reg: vec![Imm(imm)],
            o_reg: Some(r1),
            flags: flag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2
    pub fn mov(&mut self, r1: PReg, r2: PReg, flag: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Mov,
            i_reg: vec![Reg(r2)],
            o_reg: Some(r1),
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

    /// Jmp (r1 + addr)
    pub fn jmp_offset(&mut self, r1: PReg, addr: i32) {
        self.instrs.push( Instruction {
            op: Operation::JmpOff(addr),
            i_reg: vec![Reg(r1)],
            o_reg: None,
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Branch to either false_part or true_part, flags determine what kind of compare instruction
    /// is supposed to be inserted
    pub fn branch(&mut self, r2: PReg, r3: PReg, true_part: usize, false_part: usize, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Branch(true_part, false_part),
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// r1 = [r2 + off]
    pub fn load(&mut self, r1: PReg, r2: PReg, off: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Load,
            i_reg: vec![Reg(r2), Imm(off)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// [r1 + off] = r2
    pub fn store(&mut self, r1: PReg, r2: PReg, off: i32, flags: u16) {
        self.instrs.push( Instruction {
            op: Operation::Store,
            i_reg: vec![Reg(r1), Reg(r2), Imm(off)],
            o_reg: None,
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
    }

    /// Set res_reg if rs1_reg is less than imm_reg
    pub fn slt(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Slt,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// Set res_reg if rs1_reg is less than the immediate
    pub fn slti(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Slt,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 + r3
    pub fn add(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Add,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 + imm
    pub fn addi(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Add,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 - r3
    pub fn sub(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Sub,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 - imm
    pub fn subi(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Sub,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 ^ r3
    pub fn xor(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Xor,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 ^ imm
    pub fn xori(&mut self, r1: PReg, r2: PReg, imm: i32) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Xor,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 | r3
    pub fn or(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Or,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 | imm
    pub fn ori(&mut self, r1: PReg, r2: PReg, imm: i32) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Or,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 & r3
    pub fn and(&mut self, r1: PReg, r2: PReg, r3: PReg) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::And,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 & imm
    pub fn andi(&mut self, r1: PReg, r2: PReg, imm: i32) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::And,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags: Flag::NoFlag,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 << r3
    pub fn shl(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Shl,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 << imm
    pub fn shli(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Shl,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> r3 (Logical)
    pub fn shr(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Shr,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> imm (Logical)
    pub fn shri(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Shr,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> r3 (Arithmetic)
    pub fn sar(&mut self, r1: PReg, r2: PReg, r3: PReg, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Sar,
            i_reg: vec![Reg(r2), Reg(r3)],
            o_reg: Some(r1),
            flags,
            pc: self.cur_pc,
        });
        self.cur_pc = None;
        r1
    }

    /// r1 = r2 >> imm (Arithmetic)
    pub fn sari(&mut self, r1: PReg, r2: PReg, imm: i32, flags: u16) -> PReg {
        self.instrs.push( Instruction {
            op: Operation::Sar,
            i_reg: vec![Reg(r2), Imm(imm)],
            o_reg: Some(r1),
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

    /// Return a hashmap that tracks the starting pc of each cfg block of this function
    pub fn get_leaders(&self) -> FxHashMap<usize, usize> {
        let mut leader_set: FxHashMap<usize, usize> = FxHashMap::default();

        // First instruction is always a block-leader
        leader_set.insert(self.instrs[0].pc.unwrap(), 0);

        // Next insert all labels that indicate the start of a block
        for i in 0..self.instrs.len() {
            if let Some(pc) = self.instrs[i].pc {
                if self.labels.get(&pc).is_some() {
                    leader_set.insert(pc, 0);
                }
            }
        }
        leader_set
    }

    //pub fn dump_instrs_dot(&self) {
    //    let mut graph = Graph::<_, i32>::new();
    //    let mut map: FxHashMap<usize, usize> = FxHashMap::default();
    //    let mut edges: Vec<(u32, u32)> = Vec::new();

    //    for (i, instr) in self.instrs.clone().into_iter().enumerate() {
    //        match instr.op {
    //            Operation::Branch(x, _) => {
    //                if map.get(&x).is_some() {
    //                    let v = *map.get(&x).unwrap() as u32;
    //                    edges.push( (i as u32, v) );
    //                }
    //                map.insert(x, i);
    //                edges.push( (i as u32, (i + 1) as u32) );
    //            },
    //            Operation::Label(x) => {
    //                if map.get(&x).is_some() {
    //                    let v = *map.get(&x).unwrap() as u32;
    //                    edges.push( (v, (i) as u32) );
    //                }
    //                map.insert(x, i);
    //                edges.push( (i as u32, (i + 1) as u32) );
    //            },
    //            Operation::Jmp(x) => {
    //                if map.get(&x).is_some() {
    //                    let v = *map.get(&x).unwrap() as u32;
    //                    edges.push( (i as u32, v) );
    //                }
    //                map.insert(x, i);
    //            }
    //            Operation::Call(x) => {
    //                if map.get(&x).is_some() {
    //                    let v = *map.get(&x).unwrap() as u32;
    //                    edges.push( (i as u32, v) );
    //                }
    //                map.insert(x, i);
    //                edges.push( (i as u32, (i + 1) as u32) );
    //            }
    //            _ => {
    //                edges.push( (i as u32, (i + 1) as u32) );
    //            },
    //        };
    //        graph.add_node(instr);
    //    }
    //    for edge in edges.iter().take(edges.len() - 1) {
    //        graph.extend_with_edges([edge]);
    //    }
    //    let mut f = File::create("graph.dot").unwrap();
    //    let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    //    f.write_all(output.as_bytes()).expect("could not write file");
    //}

    ///// Dump a dot graph for visualization purposes
    //pub fn dump_dot(&self, name: usize) {
    //    let mut graph = Graph::<_, i32>::new();

    //    let mut s = String::new();

    //    for block in &self.blocks {
    //        s.push_str(&format!("\tLabel(0x{:x})\n\n", block.label));
    //        block.phi_funcs.iter().for_each(|e| { s.push_str(&format!("{}\n", e)); });
    //        block.instrs(&self.instrs).iter().for_each(|e| { s.push_str(&format!("{}\n", e)); });
    //        graph.add_node(s.clone());
    //        s.clear();
    //    }

    //    graph.extend_with_edges(&self.edges);

    //    let mut f = File::create(format!("graph_{:x}.dot", name)).unwrap();
    //    let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
    //    f.write_all(output.as_bytes()).expect("could not write file");
    //}
}
