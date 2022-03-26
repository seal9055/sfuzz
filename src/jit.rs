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

        // Address of function start
        let init_pc = irgraph.instrs[0].pc.unwrap();

        // Temporary registers used to load spilled registers into
        let regs_64 = [rbx, rcx];


        //TODO early return to fix race condition bug


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

        // Insert hook for addresses we want to hook with our own function and return
        if hooks.get(&init_pc).is_some() {
            jit_exit1!(3, init_pc);
            return Some(self.add_jitblock(&asm.assemble(0x0).unwrap(), init_pc));
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
            }

            match instr.op {
                Operation::Mov => {
                    let vr_out = instr.o_reg.unwrap();
                    let input  = instr.i_reg[0];
                    let r_out  = get_reg_64!(vr_out, 0);

                    // Check if input is a register or an immediate
                    match input {
                        Val::Reg(v) => {
                            let r_in = get_reg_64!(v, 1);
                            asm.mov(r_out, r_in).unwrap();
                        },
                        Val::Imm(v) => {
                            // sign/zero extend immediate
                            match instr.flags {
                                Flag::Signed   => asm.mov(r_out, v as i64).unwrap(),
                                Flag::Unsigned => asm.mov(r_out, v as u64).unwrap(),
                                _ => unreachable!(),
                            };
                        }
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_out).unwrap();
                    }
                },
                Operation::Branch(t, _f) => {
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let vr_in2 = extract_reg!(instr.i_reg[1]);
                    let mut fallthrough = asm.create_label();

                    asm.mov(rax, ptr(r14 + vr_in1.get_offset())).unwrap();
                    asm.mov(rbx, ptr(r14 + vr_in2.get_offset())).unwrap();
                    asm.cmp(rax, rbx).unwrap();

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
                            //asm.jae(fallthrough).unwrap();
                            asm.jnb(fallthrough).unwrap();
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

                    let shifted = t * 2;
                    asm.mov(rbx, ptr(r15 + shifted)).unwrap();
                    asm.jmp(rbx).unwrap();

                    asm.set_label(&mut fallthrough).unwrap();
                    asm.nop().unwrap();
                },
                Operation::Jmp(addr) => {
                    if let Some(jit_addr) = self.lookup(addr) {
                        asm.mov(rbx, jit_addr as u64).unwrap();
                        asm.jmp(rbx).unwrap();
                    } else {
                        let mut jit_exit = asm.create_label();
                        let shifted = addr * 2;
                        asm.mov(rbx, ptr(r15 + shifted)).unwrap();
                        asm.test(rbx, rbx).unwrap();
                        asm.jz(jit_exit).unwrap();
                        asm.jmp(rbx).unwrap();

                        asm.set_label(&mut jit_exit).unwrap();
                        jit_exit1!(1, addr);
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
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let vr_in2 = extract_reg!(instr.i_reg[1]);
                    let r_in1  = get_reg_64!(vr_in1, 0);
                    let offset = extract_imm!(instr.i_reg[2]);

                    asm.add(r_in1, offset).unwrap();

                    // Perform store operation with varying operand sizes based on flags
                    match instr.flags {
                        Flag::Byte => {
                            asm.mov(rcx, byte_ptr(r14 + vr_in2.get_offset())).unwrap();
                            asm.mov(byte_ptr(r13 + r_in1), cl).unwrap();
                        },
                        Flag::Word => {
                            asm.mov(rcx, word_ptr(r14 + vr_in2.get_offset())).unwrap();
                            asm.mov(word_ptr(r13 + r_in1), cx).unwrap();
                        },
                        Flag::DWord => {
                            asm.mov(rcx, dword_ptr(r14 + vr_in2.get_offset())).unwrap();
                            asm.mov(dword_ptr(r13 + r_in1), ecx).unwrap();
                        },
                        Flag::QWord => {
                            asm.mov(rcx, qword_ptr(r14 + vr_in2.get_offset())).unwrap();
                            asm.mov(qword_ptr(r13 + r_in1), rcx).unwrap();
                        },
                        _ => panic!("Unimplemented flag for store operation used"),
                    }
                },
                Operation::Load => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let r_in1  = get_reg_64!(vr_in1, 0);
                    let offset = extract_imm!(instr.i_reg[1]);

                    asm.add(r_in1, offset).unwrap();

                    // Perform load operation with varying operand sizes based on flags
                    match instr.flags {
                        0b0001000001 => {   /* Signed | Byte */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.movsx(r_out, byte_ptr(r_in1 + r13)).unwrap();
                        },
                        0b0010000001 => {   /* Signed | Word */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.movsx(r_out, word_ptr(r_in1 + r13)).unwrap();
                        },
                        0b0100000001 => {   /* Signed | DWord */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.movsxd(r_out, dword_ptr(r_in1 + r13)).unwrap();
                        },
                        0b0001000010 => {   /* Unsigned | Byte */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.movzx(r_out, byte_ptr(r_in1 + r13)).unwrap();
                        },
                        0b0010000010 => {   /* Unsigned | Word */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.movzx(r_out, word_ptr(r_in1 + r13)).unwrap();
                        },
                        0b1000000000 => {   /* QWord */
                            let r_out = get_reg_64!(vr_out, 1);
                            asm.mov(r_out, qword_ptr(r_in1 + r13)).unwrap();
                        },
                        0b0100000010 => {   /* Unsigned | DWord */
                            let r_out = get_reg_64!(vr_out, 1);

                            asm.movzx(to_32(r_out), dword_ptr(r_in1 + r13)).unwrap();

                            // Save the result of the operation if necessary
                            if vr_out.is_spilled() {
                                asm.mov(ptr(r14 + vr_out.get_offset()), rcx).unwrap();
                            }
                            continue;
                        },
                        _ => panic!("Unimplemented flag for Load operation used"),
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), rcx).unwrap();
                    }
                },
                Operation::Add => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    match instr.flags {
                        Flag::DWord => { /* 32-bit add operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.add(to_32(r_in1), to_32(r_in2)).unwrap();
                                },
                                Val::Imm(v) => {
                                    asm.add(to_32(r_in1), v).unwrap();
                                    // RISCV requires signextension on 32-bit instructions on imm's
                                    asm.movsxd(r_in1, to_32(r_in1)).unwrap();
                                },
                            }
                        },
                        Flag::QWord => { /* 64-bit add operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.add(r_in1, r_in2).unwrap();
                                },
                                Val::Imm(v) => asm.add(r_in1, v).unwrap(),
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Sub => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    match instr.flags {
                        Flag::DWord => { /* 32-bit sub operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.sub(to_32(r_in1), to_32(r_in2)).unwrap();
                                },
                                Val::Imm(v) => {
                                    asm.sub(to_32(r_in1), v).unwrap();
                                    // RISCV requires signextension on 32-bit instructions on imm's
                                    asm.movsxd(r_in1, to_32(r_in1)).unwrap();
                                },
                            }
                        },
                        Flag::QWord => { /* 64-bit sub operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.sub(r_in1, r_in2).unwrap();
                                },
                                Val::Imm(v) => asm.sub(r_in1, v).unwrap(),
                            }
                        },
                        _ => panic!("Unsupported flag provided for Sub Instruction")
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Shl => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    match instr.flags {
                        Flag::DWord => { /* 32-bit shl operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.shl(to_32(r_in1), cl).unwrap();
                                },
                                Val::Imm(v) => {
                                    asm.shl(to_32(r_in1), v).unwrap();
                                    // RISCV requires signextension on 32-bit instructions on imm's
                                    asm.movsxd(r_in1, to_32(r_in1)).unwrap();
                                },
                            }
                        },
                        Flag::QWord => { /* 64-bit shl operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.shl(r_in1, cl).unwrap();
                                },
                                Val::Imm(v) => asm.shl(r_in1, v).unwrap(),
                            }
                        },
                        _ => panic!("Unsupported flag provided for Shl Instruction")
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Shr => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    match instr.flags {
                        Flag::DWord => { /* 32-bit shr operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.shr(to_32(r_in1), cl).unwrap();
                                },
                                Val::Imm(v) => {
                                    asm.shr(to_32(r_in1), v).unwrap();
                                    // RISCV requires signextension on 32-bit instructions on imm's
                                    asm.movsxd(r_in1, to_32(r_in1)).unwrap();
                                },
                            }
                        },
                        Flag::QWord => { /* 64-bit shr operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.shr(r_in1, cl).unwrap();
                                },
                                Val::Imm(v) => asm.shr(r_in1, v).unwrap(),
                            }
                        },
                        _ => panic!("Unsupported flag provided for Shr Instruction")
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Sar => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    match instr.flags {
                        Flag::DWord => { /* 32-bit sar operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.sar(to_32(r_in1), cl).unwrap();
                                },
                                Val::Imm(v) => {
                                    asm.sar(to_32(r_in1), v).unwrap();
                                    // RISCV requires signextension on 32-bit instructions on imm's
                                    asm.movsxd(r_in1, to_32(r_in1)).unwrap();
                                },
                            }
                        },
                        Flag::QWord => { /* 64-bit sar operation */
                            // Check if input-2 is a register or an immediate
                            match in2 {
                                Val::Reg(v) => {
                                    let r_in2 = get_reg_64!(v, 1);
                                    asm.mov(rcx, r_in2).unwrap();
                                    asm.sar(r_in1, cl).unwrap();
                                },
                                Val::Imm(v) => asm.sar(r_in1, v).unwrap(),
                            }
                        },
                        _ => panic!("Unsupported flag provided for Sar Instruction")
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::And => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    // Check if input-2 is a register or an immediate
                    match in2 {
                        Val::Reg(v) => {
                            let r_in2 = get_reg_64!(v, 1);
                            asm.and(r_in1, r_in2).unwrap();
                        },
                        Val::Imm(v) => asm.and(r_in1, v).unwrap(),
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Xor => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    // Check if input-2 is a register or an immediate
                    match in2 {
                        Val::Reg(v) => {
                            let r_in2 = get_reg_64!(v, 1);
                            asm.xor(r_in1, r_in2).unwrap();
                        },
                        Val::Imm(v) => asm.xor(r_in1, v).unwrap(),
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Or => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    // Check if input-2 is a register or an immediate
                    match in2 {
                        Val::Reg(v) => {
                            let r_in2 = get_reg_64!(v, 1);
                            asm.or(r_in1, r_in2).unwrap();
                        },
                        Val::Imm(v) => asm.or(r_in1, v).unwrap(),
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), r_in1).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), r_in1).unwrap();
                        }
                    }
                },
                Operation::Slt => {
                    let vr_out = instr.o_reg.unwrap();
                    let vr_in1 = extract_reg!(instr.i_reg[0]);
                    let in2    = instr.i_reg[1];
                    let r_in1  = get_reg_64!(vr_in1, 0);

                    // Need an extra register for this operation, use r15 and restore it after instr
                    asm.push(r15).unwrap();
                    asm.mov(r15, r_in1).unwrap();

                    asm.xor(ecx, ecx).unwrap();

                    // Check if input-2 is a register or an immediate
                    match in2 {
                        Val::Reg(v) => {
                            let r_in2 = get_reg_64!(v, 0);
                            asm.cmp(r15, r_in2).unwrap();
                        },
                        Val::Imm(v) => asm.cmp(r15, v).unwrap(),
                    }

                    // Check if operation is Signed or Unsigned
                    match instr.flags {
                        Flag::Signed   => asm.setl(cl).unwrap(),
                        Flag::Unsigned => asm.setb(cl).unwrap(),
                        _ => unreachable!(),
                    }

                    // Save the result of the operation if necessary
                    if vr_out.is_spilled() {
                        asm.mov(ptr(r14 + vr_out.get_offset()), rcx).unwrap();
                    } else {
                        if vr_out != vr_in1 {
                            asm.mov(vr_out.convert_64(), rcx).unwrap();
                        }
                    }
                    asm.pop(r15).unwrap();
                },
                Operation::Syscall => {
                    jit_exit1!(2, instr.pc.unwrap() + 4);
                }
                _ => panic!("unimplemented instr: {:?}", instr),
            }
        }

        // Actually compile the function and return the address it is compiled at
        Some(self.add_jitblock(&asm.assemble(0x0).unwrap(), init_pc))
    }
}

#[allow(non_upper_case_globals)]
fn to_32(reg: AsmRegister64) -> AsmRegister32 {
    match reg {
        rax => eax,
        rbx => ebx,
        rcx => ecx,
        _ => unreachable!(),
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
