use crate::{
    irgraph::{IRGraph, Flag, Operation, Val},
    emulator::{Emulator, Fault},
};

use rustc_hash::FxHashMap;
use iced_x86::code_asm::*;

use std::sync::Mutex;
use std::sync::RwLock;

/// Allocate RWX memory for Linux systems
#[cfg(target_os="linux")]
pub fn alloc_rwx(size: usize) -> &'static mut [u8] {
    extern {
        fn mmap(addr: *mut u8, length: usize, prot: i32, flags: i32, fd: i32,
                offset: usize) -> *mut u8;
    }

    unsafe {
        // Alloc RWX and MAP_PRIVATE | MAP_ANON
        let ret = mmap(std::ptr::null_mut::<u8>(), size, 7, 34, -1, 0);
        assert!(!ret.is_null());

        std::slice::from_raw_parts_mut(ret, size)
    }
}

/// Holds various information related to tracking statistics for the fuzzer
#[derive(Default, Debug)]
pub struct Statistics {
    pub total_cases: usize,
}

/// Holds the backing that contains the just-in-time compiled code
#[derive(Debug)]
pub struct Jit {
    pub jit_backing: Mutex<(&'static mut [u8], usize)>,

    pub lookup_arr: RwLock<Vec<usize>>,

    // TODO move stats out of here and into messages
    pub stats: Mutex<Statistics>,
}

impl Jit {
    /// Create a new JIT memory space. Should only be used once and then shared between threads
    pub fn new(address_space_size: usize) -> Self {
        Jit {
            jit_backing: Mutex::new((alloc_rwx(16*1024*1024), 0)),
            lookup_arr: RwLock::new(vec![0; address_space_size / 4]),
            stats: Mutex::new(Statistics::default()),
        }
    }

    /// Write opcodes to the JIT backing buffer and add a mapping to lookup table
    pub fn add_jitblock(&self, code: &[u8], pc: usize) -> usize {
        let mut jit = self.jit_backing.lock().unwrap();

        let jit_inuse = jit.1;
        jit.0[jit_inuse..jit_inuse + code.len()].copy_from_slice(code);

        let addr = jit.0.as_ptr() as usize + jit_inuse;

        // add mapping
        self.lookup_arr.write().unwrap()[pc / 4] = addr;

        jit.1 += code.len();

        // Return the JIT address of the code we just compiled
        addr
    }

    /// Look up jit address corresponding to a translated instruction
    pub fn lookup(&self, pc: usize) -> Option<usize> {
        let addr = self.lookup_arr.read().unwrap()[pc / 4];
        if addr == 0 {
            Option::None
        } else {
            Some(addr)
        }
    }

    /// Add a new mapping to the lookup table without actually inserting code into jit
    pub fn add_lookup(&self, code: &[u8], pc: usize) {
        let jit = self.jit_backing.lock().unwrap();

        let cur_jit_addr = jit.0.as_ptr() as usize + jit.1;

        self.lookup_arr.write().unwrap()[pc / 4] = cur_jit_addr + code.len();
    }

    /// r12 : contains pointer to permissions map
    /// r13 : contains pointer to memory map
    /// r14 : contains pointer to memory mapped register array
    /// r15 : contains pointer to jit lookup array
    pub fn compile(&self, irgraph: &IRGraph, hooks: &FxHashMap<usize, fn(&mut Emulator) 
            -> Result<(), Fault>>) -> Option<usize> {

        let mut asm = CodeAssembler::new(64).unwrap();
        let init_pc = irgraph.instrs[0].pc.unwrap();
        let regs_64 = [rbx, rcx];
        let regs_32 = [ebx, ecx];

        println!("################################################");
        println!("Compiling: 0x{:x}", init_pc);
        println!("################################################");

        /// Returns the destination register for an operation
        macro_rules! get_reg_64 {
            ($reg: expr, $i: expr) => {
                if $reg.is_spilled() {
                    asm.mov(regs_64[$i], ptr(r14 + $reg.get_offset())).unwrap();
                    regs_64[$i]
                } else {
                    $reg.convert_64()
                }
            }
        }

        /// Returns the destination register for an operation
        macro_rules! get_reg_32 {
            ($reg: expr, $i: expr) => {
                if $reg.is_spilled() {
                    asm.mov(regs_32[$i], ptr(r14 + $reg.get_offset())).unwrap();
                    regs_32[$i]
                } else {
                    $reg.convert_32()
                }
            }
        }

        /// Forcibly extract a register from the `Val` enum
        macro_rules! extract_reg {
            ($reg: expr) => {
                match $reg {
                    Val::Reg(v) => v,
                    Val::Imm(_) => panic!("extract_reg called with an immediate")
                }
            }
        }

        /// Forcibly extract an immediate from the `Val` enum
        macro_rules! extract_imm {
            ($reg: expr) => {
                match $reg {
                    Val::Reg(_) => panic!("extract_imm called with a register"),
                    Val::Imm(v) => v
                }
            }
        }

        /// Jit exit with reentry address stored in an immediate 
        macro_rules! jit_exit1 {
            ($code: expr, $reentry: expr) => {
                asm.mov(rax, $code as u64).unwrap();
                asm.mov(rcx, $reentry as u64).unwrap();
                asm.ret().unwrap();
            }
        }

        /// Jit exit with reentry address stored in a register
        macro_rules! jit_exit2 {
            ($code: expr, $reentry: expr) => {
                asm.mov(rax, $code as u64).unwrap();
                asm.mov(rcx, $reentry).unwrap();
                asm.ret().unwrap();
            }
        }

        let mut first = true;

        for instr in &irgraph.instrs {
            if let Some(pc) = instr.pc {
                if first {
                    first = false;
                } else {
                    //jit_exit1!(4, pc);
                }

                self.add_lookup(&asm.assemble(0x0).unwrap(), pc);
                println!("0x{:X} -> 0x{:X}", pc, self.lookup(pc).unwrap());
            }

            match instr.op {
                Operation::MovI(v) => {
                    let r1 = instr.o_reg.unwrap();
                    let reg = get_reg_64!(r1, 0);

                    // sign/zero extend immediate
                    let extended = match instr.flags {
                        Flag::Signed => {
                            v as i64 as u64
                        },
                        Flag::Unsigned => {
                            v as u64
                        }
                        _ => unreachable!(),
                    };
                    asm.mov(reg, extended as u64).unwrap();

                    if r1.is_spilled() {
                        asm.mov(ptr(r14 + r1.get_offset()), reg).unwrap();
                    }
                },
                Operation::Mov => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in  = instr.i_reg[0];

                    let preg_out = get_reg_64!(reg_out, 0);

                    match reg_in {
                        Val::Reg(v) => {
                            let v = get_reg_64!(v, 1);
                            asm.mov(preg_out, v).unwrap();
                        },
                        Val::Imm(v) => {
                            // sign/zero extend immediate
                            let extended = match instr.flags {
                                Flag::Signed => {
                                    v as i64 as u64
                                },
                                Flag::Unsigned => {
                                    v as u64
                                }
                                _ => unreachable!(),
                            };
                            asm.mov(preg_out, extended).unwrap();
                        }
                    }

                    if reg_out.is_spilled() {
                        asm.mov(ptr(r14 + reg_out.get_offset()), preg_out).unwrap();
                    }
                },
                Operation::Branch(t, _f) => {
                    let r1 = get_reg_64!(extract_reg!(instr.i_reg[0]), 0);
                    let r2 = get_reg_64!(extract_reg!(instr.i_reg[1]), 1);
                    let mut fallthrough = asm.create_label();

                    asm.cmp(r1, r2).unwrap();

                    match instr.flags {
                        0b000101 => {   /* Signed | Equal */
                            asm.jne(fallthrough).unwrap();
                        },
                        0b001001 => {   /* Signed | NEqual */
                            asm.je(fallthrough).unwrap();
                        },
                        0b010001 => {   /* Signed | Less */
                            asm.jnl(fallthrough).unwrap();
                        },
                        0b100001 => {   /* Signed | Greater */
                            asm.jng(fallthrough).unwrap();
                        },
                        0b010101 => {   /* Signed | Less | Equal */
                            asm.jnle(fallthrough).unwrap();
                        },
                        0b100101 => {   /* Signed | Greater | Equal */
                            asm.jnge(fallthrough).unwrap();
                        },
                        0b010010 => {   /* Unsigned | Less */
                            asm.jae(fallthrough).unwrap();
                        },
                        0b100010 => {   /* Unsigned | Greater */
                            asm.jna(fallthrough).unwrap();
                        },
                        0b010110 => {   /* Unsigned | Less | Equal */
                            asm.jnbe(fallthrough).unwrap();
                        },
                        0b100110 => {   /* Unsigned | Greater | Equal */
                            asm.jnae(fallthrough).unwrap();
                        },
                        _ => panic!("Unimplemented conditional branch flags")
                    }

                    asm.mov(rbx, ptr(r15 + (t * 2))).unwrap();
                    asm.jmp(rbx).unwrap();

                    asm.set_label(&mut fallthrough).unwrap();
                    asm.nop().unwrap();
                },
                Operation::Jmp(addr) => {
                    // Insert hook if we attempt to jmp to a hooked function
                    if hooks.get(&addr).is_some() {
                        jit_exit1!(3, addr);
                        panic!("HOOK HIT");
                    } else {
                        if let Some(jit_addr) = self.lookup(addr) {
                            asm.mov(rbx, jit_addr as u64).unwrap();
                            asm.jmp(rbx).unwrap();
                        } else {
                            let mut jit_exit = asm.create_label();
                            asm.mov(rbx, ptr(r15 + (addr * 2))).unwrap();
                            asm.test(rbx, rbx).unwrap();
                            asm.jz(jit_exit).unwrap();
                            asm.jmp(rbx).unwrap();

                            asm.set_label(&mut jit_exit).unwrap();
                            jit_exit1!(1, addr);
                        }
                    }
                },
                Operation::JmpOff(addr) => {
                    let mut jit_exit = asm.create_label();
                    let reg = get_reg_64!(extract_reg!(instr.i_reg[0]), 0);

                    asm.add(reg, addr as i32).unwrap();
                    asm.shl(reg, 1u32).unwrap();
                    asm.mov(rcx, ptr(r15 + reg)).unwrap();
                    asm.test(rcx, rcx).unwrap();
                    asm.jz(jit_exit).unwrap();
                    asm.jmp(rcx).unwrap();

                    asm.set_label(&mut jit_exit).unwrap();
                    asm.shr(reg, 1u32).unwrap();
                    jit_exit2!(1, reg);
                },
                Operation::Store => {
                    let r1   = get_reg_64!(extract_reg!(instr.i_reg[0]), 0);
                    let r2   = get_reg_64!(extract_reg!(instr.i_reg[1]), 1);
                    let offset = extract_imm!(instr.i_reg[2]);

                    asm.add(r1, r13).unwrap();

                    match instr.flags {
                        Flag::Byte => {
                            asm.mov(byte_ptr(r1 + offset), r2).unwrap();
                        }
                        Flag::Word => {
                            asm.mov(word_ptr(r1 + offset), r2).unwrap();
                        }
                        Flag::DWord => {
                            asm.mov(dword_ptr(r1 + offset), r2).unwrap();
                        }
                        Flag::QWord => {
                            asm.mov(qword_ptr(r1 + offset), r2).unwrap();
                        }
                        _ => panic!("Unimplemented flag for store operation used"),
                    }
                },
                Operation::Load => {
                    let r1 = instr.o_reg.unwrap();
                    let r2 = get_reg_64!(extract_reg!(instr.i_reg[0]), 0);
                    let offset = extract_imm!(instr.i_reg[1]);

                    asm.add(r2, r13).unwrap();

                    match instr.flags {
                        0b0001000001 => {   /* Signed | Byte */
                            let r1 = get_reg_64!(r1, 1);
                            asm.movsx(r1, byte_ptr(r2 + offset)).unwrap();
                        },
                        0b0010000001 => {   /* Signed | Word */
                            let r1 = get_reg_64!(r1, 1);
                            asm.movsx(r1, word_ptr(r2 + offset)).unwrap();
                        },
                        0b0100000001 => {   /* Signed | DWord */
                            let r1 = get_reg_64!(r1, 1);
                            asm.movsxd(r1, dword_ptr(r2 + offset)).unwrap();
                        },
                        0b0001000010 => {   /* Unsigned | Byte */
                            let r1 = get_reg_64!(r1, 1);
                            asm.movzx(r1, byte_ptr(r2 + offset)).unwrap();
                        },
                        0b0010000010 => {   /* Unsigned | Word */
                            let r1 = get_reg_64!(r1, 1);
                            asm.movzx(r1, word_ptr(r2 + offset)).unwrap();
                        },
                        0b0100000010 => {   /* Unsigned | DWord */
                            let reg1 = get_reg_32!(extract_reg!(instr.i_reg[0]), 1);
                            asm.movzx(reg1, dword_ptr(r2 + offset)).unwrap();
                            if r1.is_spilled() {
                                asm.mov(ptr(r14 + r1.get_offset()), reg1).unwrap();
                            }
                            continue;
                        },
                        0b1000000000 => {   /* QWord */
                            let r1 = get_reg_64!(r1, 1);
                            asm.mov(r1, qword_ptr(r2 + offset)).unwrap();
                        },
                        _ => panic!("Unimplemented flag for Load operation used"),
                    }

                    if r1.is_spilled() {
                        asm.mov(ptr(r14 + r1.get_offset()), rcx).unwrap();
                    }
                },
                Operation::Add => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    match instr.flags {
                        Flag::DWord => {
                            let r1 = get_reg_32!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_32!(v, 1);
                                    asm.add(r1, tmp).unwrap();
                                },
                                Val::Imm(v) => asm.add(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_32(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1 = get_reg_64!(reg_in1, 0); // rbx

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_64!(v, 1);
                                    asm.add(r1, tmp).unwrap();
                                },
                                Val::Imm(v) => asm.add(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_64(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::Sub => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    match instr.flags {
                        Flag::DWord => {
                            let r1 = get_reg_32!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => { 
                                    let tmp = get_reg_32!(v, 1);
                                    asm.sub(r1, tmp).unwrap();
                                }
                                Val::Imm(v) => asm.sub(r1, v).unwrap(),
                            }
                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_32(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1 = get_reg_64!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_64!(v, 1);
                                    asm.sub(r1, tmp).unwrap();
                                }
                                Val::Imm(v) => asm.sub(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_64(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::Shl => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    match instr.flags {
                        Flag::DWord => {
                            let r1 = get_reg_32!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_32!(v, 1);
                                    asm.mov(ecx, tmp).unwrap();
                                    asm.shl(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.shl(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_32(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1 = get_reg_64!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_64!(v, 1);
                                    asm.mov(rcx, tmp).unwrap();
                                    asm.shl(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.shl(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_64(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::Shr => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    match instr.flags {
                        Flag::DWord => {
                            let r1 = get_reg_32!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_32!(v, 1);
                                    asm.mov(ecx, tmp).unwrap();
                                    asm.shr(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.shr(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_32(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1 = get_reg_64!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_64!(v, 1);
                                    asm.mov(rcx, tmp).unwrap();
                                    asm.shr(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.shr(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_64(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::Sar => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    match instr.flags {
                        Flag::DWord => {
                            let r1 = get_reg_32!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_32!(v, 1);
                                    asm.mov(ecx, tmp).unwrap();
                                    asm.sar(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.sar(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_32(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1 = get_reg_64!(reg_in1, 0);

                            match reg_in2 {
                                Val::Reg(v) => {
                                    let tmp = get_reg_64!(v, 1);
                                    asm.mov(rcx, tmp).unwrap();
                                    asm.sar(r1, cl).unwrap();
                                }
                                Val::Imm(v) => asm.sar(r1, v).unwrap(),
                            }

                            if reg_out != reg_in1 {
                                if reg_out.is_spilled() {
                                    asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                                } else {
                                    asm.mov(reg_out.convert_64(), r1).unwrap();
                                }
                            } else if reg_in1.is_spilled() {
                                asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::And => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    let r1 = get_reg_64!(reg_in1, 0);

                    match reg_in2 {
                        Val::Reg(v) => {
                            let tmp = get_reg_64!(v, 1);
                            asm.and(r1, tmp).unwrap();
                        }
                        Val::Imm(v) => asm.and(r1, v).unwrap(),
                    }

                    if reg_out != reg_in1 {
                        if reg_out.is_spilled() {
                            asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                        } else {
                            asm.mov(reg_out.convert_64(), r1).unwrap();
                        }
                    } else if reg_in1.is_spilled() {
                        asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                    }
                },
                Operation::Xor => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    let r1 = get_reg_64!(reg_in1, 0);

                    match reg_in2 {
                        Val::Reg(v) => {
                            let tmp = get_reg_64!(v, 1);
                            asm.xor(r1, tmp).unwrap();
                        }
                        Val::Imm(v) => asm.xor(r1, v).unwrap(),
                    }

                    if reg_out != reg_in1 {
                        if reg_out.is_spilled() {
                            asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                        } else {
                            asm.mov(reg_out.convert_64(), r1).unwrap();
                        }
                    } else if reg_in1.is_spilled() {
                        asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                    }
                },
                Operation::Or => {
                    let reg_out = instr.o_reg.unwrap();
                    let reg_in1 = extract_reg!(instr.i_reg[0]);
                    let reg_in2 = instr.i_reg[1];

                    let r1 = get_reg_64!(reg_in1, 0);

                    match reg_in2 {
                        Val::Reg(v) => {
                            let tmp = get_reg_64!(v, 1);
                            asm.or(r1, tmp).unwrap();
                        }
                        Val::Imm(v) => asm.or(r1, v).unwrap(),
                    }

                    if reg_out != reg_in1 {
                        if reg_out.is_spilled() {
                            asm.mov(ptr(r14 + reg_out.get_offset()), r1).unwrap();
                        } else {
                            asm.mov(reg_out.convert_64(), r1).unwrap();
                        }
                    } else if reg_in1.is_spilled() {
                        asm.mov(ptr(r14 + reg_in1.get_offset()), r1).unwrap();
                    }
                },
                _ => panic!("unimplemented instr: {:?}", instr),
            }
        }

        // Actually compile the function and return the address it is compiled at
        Some(self.add_jitblock(&asm.assemble(0x0).unwrap(), init_pc))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::arch::asm;

    #[test]
    fn add_lookup_test() {
        let jit = Jit::new(16 * 1024 * 1024);
        let mut asm = CodeAssembler::new(64).unwrap();

        asm.add(rax, rax).unwrap();
        asm.sub(rax, rax).unwrap();
        asm.ret().unwrap();

        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x1234);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x4444);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x9055);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x1000);

        jit.lookup(0x1234).unwrap();
        jit.lookup(0x4444).unwrap();
        jit.lookup(0x9055).unwrap();
        jit.lookup(0x1000).unwrap();
    }

    #[test]
    fn add_jitblock_test() {
        let jit = Jit::new(16 * 1024 * 1024);
        let mut asm = CodeAssembler::new(64).unwrap();

        asm.add(rax, rax).unwrap();
        asm.sub(rax, rax).unwrap();
        asm.ret().unwrap();

        jit.add_jitblock(&asm.assemble(0x0).unwrap(), 0x1234);
        jit.add_jitblock(&asm.assemble(0x0).unwrap(), 0x4444);
        jit.add_jitblock(&asm.assemble(0x0).unwrap(), 0x9055);
        jit.add_jitblock(&asm.assemble(0x0).unwrap(), 0x1000);

        jit.lookup(0x1234).unwrap();
        jit.lookup(0x4444).unwrap();
        jit.lookup(0x9055).unwrap();
        jit.lookup(0x1000).unwrap();
    }

    #[test]
    fn asm_lookup() {
        let jit = Jit::new(16 * 1024 * 1024);
        let mut asm = CodeAssembler::new(64).unwrap();
        let mut result1: usize;
        let mut result2: usize;
        let mut result3: usize;
        let mut result4: usize;

        asm.add(rax, rax).unwrap();
        asm.sub(rax, rax).unwrap();
        asm.ret().unwrap();

        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x1234);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x4444);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x9055);
        jit.add_lookup(&asm.assemble(0x0).unwrap(), 0x1000);

        unsafe {
                asm!(r#"
                    mov r8,  [r15 + 0x1234*2]
                    mov r9,  [r15 + 0x4444*2]
                    mov r10, [r15 + 0x9055*2]
                    mov r11, [r15 + 0x1000*2]
                "#,
                out("r8") result1,
                out("r9") result2,
                out("r10") result3,
                out("r11") result4,
                in("r15") jit.lookup_arr.read().unwrap().as_ptr() as u64,
                );
        }

        assert_ne!(result1, 0);
        assert_ne!(result2, 0);
        assert_ne!(result3, 0);
        assert_ne!(result4, 0);
    }
}
