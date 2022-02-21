use crate::{
    ssa_builder::SSABuilder,
    irgraph::{Flag, Operation, Reg, Instruction},
    emulator::Register as RVReg,
};

use std::sync::Mutex;
use std::sync::RwLock;

use rustc_hash::FxHashMap;

use iced_x86::code_asm::*;
use iced_x86::Register::*;
use iced_x86::Register;
use iced_x86::{Formatter, Instruction as ice_instr, NasmFormatter};

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

/// Enum used to return various types of registers
#[derive(Debug)]
enum Either {
    _64(AsmRegister64),
    _32(AsmRegister32),
    _16(AsmRegister16),
    _8(AsmRegister8),
}

impl Into<AsmRegister64> for Either {
    fn into(self) -> AsmRegister64 {
        match self {
            Either::_64(v) => v,
            _ => unreachable!(),
        }
    }
}

impl Into<AsmRegister32> for Either {
    fn into(self) -> AsmRegister32 {
        match self {
            Either::_32(v) => v,
            _ => unreachable!(),
        }
    }
}

impl Into<AsmRegister16> for Either {
    fn into(self) -> AsmRegister16 {
        match self {
            Either::_16(v) => v,
            _ => unreachable!(),
        }
    }
}

impl Into<AsmRegister8> for Either {
    fn into(self) -> AsmRegister8 {
        match self {
            Either::_8(v) => v,
            _ => unreachable!(),
        }
    }
}

/// Return actual register for a register enum
fn convert_reg(reg: Register, size: u8) -> Either {
    match size {
        64 => {
            match reg {
                RAX => Either::_64(rax),
                RBX => Either::_64(rbx),
                RCX => Either::_64(rcx),
                RDX => Either::_64(rdx),
                RDI => Either::_64(rdi),
                RSI => Either::_64(rsi),
                RSP => Either::_64(rsp),
                RBP => Either::_64(rbp),
                R8  => Either::_64(r8),
                R9  => Either::_64(r9),
                R10 => Either::_64(r10),
                R11 => Either::_64(r11),
                R12 => Either::_64(r12),
                R13 => Either::_64(r13),
                R14 => Either::_64(r14),
                R15 => Either::_64(r15),
                None => Either::_64(r15), /* used to indicate spilling since r15 isnt used */
                _ => unreachable!(),
            }
        },
        32 => {
            match reg {
                RAX => Either::_32(eax),
                RBX => Either::_32(ebx),
                RCX => Either::_32(ecx),
                RDX => Either::_32(edx),
                RDI => Either::_32(edi),
                RSI => Either::_32(esi),
                RSP => Either::_32(esp),
                RBP => Either::_32(ebp),
                R8  => Either::_32(r8d),
                R9  => Either::_32(r9d),
                R10 => Either::_32(r10d),
                R11 => Either::_32(r11d),
                R12 => Either::_32(r12d),
                R13 => Either::_32(r13d),
                R14 => Either::_32(r14d),
                R15 => Either::_32(r15d),
                None => Either::_32(r15d), /* used to indicate spilling since r15 isnt used */
                _ => unreachable!(),
            }
        },
        16 => {
            match reg {
                RAX => Either::_16(ax),
                RBX => Either::_16(bx),
                RCX => Either::_16(cx),
                RDX => Either::_16(dx),
                RDI => Either::_16(di),
                RSI => Either::_16(si),
                RSP => Either::_16(sp),
                RBP => Either::_16(bp),
                R8  => Either::_16(r8w),
                R9  => Either::_16(r9w),
                R10 => Either::_16(r10w),
                R11 => Either::_16(r11w),
                R12 => Either::_16(r12w),
                R13 => Either::_16(r13w),
                R14 => Either::_16(r14w),
                R15 => Either::_16(r15w),
                None => Either::_16(r15w), /* used to indicate spilling since r15 isnt used */
                _ => unreachable!(),
            }
        },
        8 => {
            match reg {
                RAX => Either::_8(al),
                RBX => Either::_8(bl),
                RCX => Either::_8(cl),
                RDX => Either::_8(dl),
                RDI => Either::_8(sil),
                RSI => Either::_8(dil),
                RSP => Either::_8(spl),
                RBP => Either::_8(bpl),
                R8  => Either::_8(r8b),
                R9  => Either::_8(r9b),
                R10 => Either::_8(r10b),
                R11 => Either::_8(r11b),
                R12 => Either::_8(r12b),
                R13 => Either::_8(r13b),
                R14 => Either::_8(r14b),
                R15 => Either::_8(r15b),
                None => Either::_8(r15b), /* used to indicate spilling since r15 isnt used */
                _ => unreachable!(),
            }
        },
        _ => panic!("Unsupported register size. Supported sizes are: {{8, 16, 32 & 64}}"),
    }
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
    /// Create a new JIT memory space. Should only be used once and then shared between threads.
    pub fn new(address_space_size: usize) -> Self {
        Jit {
            jit_backing: Mutex::new((alloc_rwx(16*1024*1024), 0)),
            lookup_arr: RwLock::new(vec![0; address_space_size / 4]),
            stats: Mutex::new(Statistics::default()),
        }
    }

    /// Probably gonna remove this
    pub fn add_jitblock(&self, code: &[u8], pc: usize) -> usize {
        let mut jit = self.jit_backing.lock().unwrap();

        let jit_inuse = jit.1;
        jit.0[jit_inuse..jit_inuse + code.len()].copy_from_slice(code);

        let addr = jit.0.as_ptr() as usize + jit_inuse;

        // add mapping
        self.lookup_arr.write().unwrap()[pc] = addr;

        jit.1 += code.len();

        // Return the JIT address of the code we just compiled
        addr
    }

    /// Get the mapping of a pc from the original code to the compiled code in the jit
    pub fn lookup(&self, pc: usize) -> Option<usize> {
        let addr = self.lookup_arr.read().unwrap()[pc];
        if addr == 0 {
            Option::None
        } else {
            Some(addr)
        }
    }

    /// Add a new mapping to the loopup array
    fn add_lookup(&self, code: &[u8], pc: usize) {
        let jit = self.jit_backing.lock().unwrap();

        let cur_jit_addr = jit.0.as_ptr() as usize + jit.1;

        self.lookup_arr.write().unwrap()[pc] = cur_jit_addr + code.len();
    }

    /// Return the register that is used furthest in the future. Used to spill registers into memory
    /// when necessary
    fn furthest_reg(instrs: &[Instruction], reg_mapping: &FxHashMap<Reg, Register>) -> Register {
        let mut regs: Vec<Option<(usize, Register)>> = vec![Option::None; 100];

        for (i, instr) in instrs.iter().enumerate() {
            for input in &instr.i_reg {
                let reg: Register = *reg_mapping.get(&input).unwrap();

                if regs[reg as usize].is_none() {
                    regs[reg as usize] = Some((i, reg));
                }
            }
            if let Some(output) = instr.o_reg {
                let reg: Register = *reg_mapping.get(&output).unwrap();

                if regs[reg as usize].is_none() {
                    regs[reg as usize] = Some((i, reg));
                }
            }
        }
        // Determine the register at the max position
        regs.iter().max().unwrap().unwrap().1
    }

    fn get_reg_offset(&self, reg: RVReg) -> usize {
        reg as usize * 8
    }

    /// Compile an SSA form CFG into linear x86 machine code
    pub fn compile(&self, ssa: &SSABuilder, reg_mapping: &FxHashMap<Reg, Register>,
                   labels: Vec<usize>) -> Option<usize> {

        let mut asm = CodeAssembler::new(64).unwrap();
        let mut label_map: FxHashMap<usize, CodeLabel> = FxHashMap::default();
        let mut pc = ssa.instrs[0].pc.unwrap();
        let init_pc = pc;

        for label in labels {
            label_map.insert(label, asm.create_label());
        }

        // Function prologue {{{

            // Restore stack pointer
        let sp_off = self.get_reg_offset(RVReg::Sp);
        asm.mov(rbp, ptr(r15)).unwrap();
        asm.mov(rbp, ptr(rbp+sp_off)).unwrap();

        // }}}

        macro_rules! set_reg {
            ($reg: expr, $src_reg: expr, $asm: expr) => {
                let dst_off = self.get_reg_offset($reg);
                asm.mov(ptr(ptr(r15)+dst_off), $src_reg).unwrap();
            }
        }

        macro_rules! function_epilogue1 {
            ($code: expr, $reentry: expr, $asm: expr) => {
                set_reg!(RVReg::Sp, rbp, asm);
                asm.mov(rax, $code as u64).unwrap();
                asm.mov(rcx, $reentry as u64).unwrap();
                asm.ret().unwrap();
            }
        }

        macro_rules! function_epilogue2 {
            ($code: expr, $reentry: expr, $asm: expr) => {
                set_reg!(RVReg::Sp, rbp, asm);
                asm.mov(rax, $code as u64).unwrap();
                asm.mov(rcx, $reentry).unwrap();
                asm.ret().unwrap();
            }
        }

        println!("label_map: {:#x?}", label_map);

        for instr in &ssa.instrs {

            Jit::furthest_reg(&ssa.instrs, &reg_mapping);

            // Insert label if this is the start of a new block
            if instr.pc.is_some() {
                if let Some(label) = label_map.get(&instr.pc.unwrap()) {
                    let mut tmp = label.clone();

                    // Add a mapping of the previous block to its jit address to the lookup array.
                    // These mappings are important so that jump targets that can't be precomputed
                    // by labels eg. `jmp rax`, can make use of the lookup table to find their 
                    // target.
                    pc = instr.pc.unwrap();
                    self.add_lookup(&asm.assemble(0x0).unwrap(), pc);

                    asm.set_label(&mut tmp).unwrap();
                }
            }

            match instr.op {
                Operation::Loadi(v) => {
                    let reg: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.o_reg.unwrap())
                                                         .unwrap(), 64).into();
                    // sign extend immediate
                    let extended = match instr.flags {
                        Flag::Signed => {
                            v as i64 as u64
                        },
                        Flag::Unsigned => {
                            v as u64
                        }
                        _ => unreachable!(),
                    };
                    asm.mov(reg, extended).unwrap();
                },
                Operation::Branch(t, _f) => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                        .unwrap(), 64).into();
                    asm.cmp(r1, r2).unwrap();

                    let true_label = label_map.get(&t).unwrap();

                    if instr.flags & (Flag::Signed | Flag::Equal)
                        == Flag::Signed | Flag::Equal {
                        asm.je(*true_label).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::NEqual)
                        == (Flag::Signed | Flag::NEqual) {
                        asm.jne(*true_label).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::Less)
                        == (Flag::Signed | Flag::Less) {
                        asm.jl(*true_label).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::Less | Flag::Equal)
                        == (Flag::Signed | Flag::Less | Flag::Equal) {
                        asm.jle(*true_label).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::Greater)
                        == (Flag::Signed | Flag::Greater) {
                        asm.jg(*true_label).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::Greater | Flag::Equal)
                        == (Flag::Signed | Flag::Greater | Flag::Equal) {
                        asm.jge(*true_label).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Less)
                        == (Flag::Unsigned | Flag::Less) {
                        asm.jnae(*true_label).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Less | Flag::Equal)
                        == (Flag::Unsigned | Flag::Less | Flag::Equal) {
                        asm.jbe(*true_label).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Greater)
                        == (Flag::Unsigned | Flag::Greater) {
                        asm.ja(*true_label).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Greater | Flag::Equal)
                        == (Flag::Unsigned | Flag::Greater | Flag::Equal) {
                        asm.jae(*true_label).unwrap();
                    } else {
                        panic!("Unimplemented conditional branch flags");
                    }
                },
                Operation::Store => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                        .unwrap(), 64).into();

                    // Use lookup table to get actual memory address
                    asm.mov(rcx, ptr(r15+0x8u64)).unwrap();
                    asm.add(r2, rcx).unwrap();

                    match instr.flags {
                        Flag::Byte => {
                            asm.mov(byte_ptr(r2), r1).unwrap();
                        }
                        Flag::Word => {
                            asm.mov(word_ptr(r2), r1).unwrap();
                        }
                        Flag::DWord => {
                            asm.mov(dword_ptr(r2), r1).unwrap();
                        }
                        Flag::QWord => {
                            asm.mov(qword_ptr(r2), r1).unwrap();
                        }
                        _ => panic!("Unimplemented flag for store operation used"),
                    }
                },
                Operation::Load => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.o_reg.unwrap())
                                                        .unwrap(), 64).into();

                    // Use lookup table to get actual memory address
                    asm.mov(rcx, ptr(r15+0x8u64)).unwrap();
                    asm.add(r2, rcx).unwrap();

                    if instr.flags & (Flag::Signed | Flag::Byte) 
                        == Flag::Signed | Flag::Byte {
                            asm.movsx(r2, byte_ptr(r1)).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::Word) 
                        == Flag::Signed | Flag::Word {
                            asm.movsx(r2, word_ptr(r1)).unwrap();
                    } else if instr.flags & (Flag::Signed | Flag::DWord) 
                        == Flag::Signed | Flag::DWord {
                            asm.movsxd(r2, dword_ptr(r1)).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Byte) 
                        == Flag::Unsigned | Flag::Byte {
                            asm.movzx(r2, byte_ptr(r1)).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::Word) 
                        == Flag::Unsigned | Flag::Word {
                            asm.movzx(r2, word_ptr(r1)).unwrap();
                    } else if instr.flags & (Flag::Unsigned | Flag::DWord) 
                        == Flag::Unsigned | Flag::DWord {
                            let r2: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.o_reg
                                                                .unwrap()).unwrap(), 32).into();
                            asm.movzx(r2, dword_ptr(r1)).unwrap();
                    } else if instr.flags == Flag::QWord {
                            asm.mov(r2, qword_ptr(r1)).unwrap();
                    } else {
                        panic!("Unimplemented flag for Load operation used");
                    }
                },
                Operation::Add => {
                    match instr.flags {
                        Flag::DWord => {
                            let r1: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 32).into();
                            let r2: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 32).into();
                            let r3: AsmRegister32 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 32).into();
                            asm.add(r1, r2).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 64).into();
                            let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 64).into();
                            let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 64).into();
                            asm.add(r1, r2).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Add Instruction")
                    }
                },
                Operation::Sub => {
                    match instr.flags {
                        Flag::DWord => {
                            let r1: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 32).into();
                            let r2: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 32).into();
                            let r3: AsmRegister32 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 32).into();
                            asm.sub(r1, r2).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 64).into();
                            let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 64).into();
                            let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 64).into();
                            asm.sub(r1, r2).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Sub Instruction")
                    }
                },
                Operation::Shl => {
                    match instr.flags {
                        Flag::DWord => {
                            let r1: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 32).into();
                            let r2: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 32).into();
                            let r3: AsmRegister32 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 32).into();
                            asm.mov(ecx, r2).unwrap();
                            asm.shl(r1, cl).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        Flag::QWord => {
                            let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 64).into();
                            let r2: AsmRegister32 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                                .unwrap(), 32).into();
                            let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                                .get(&instr.o_reg.unwrap())
                                                                .unwrap(), 64).into();
                            asm.mov(ecx, r2).unwrap();
                            asm.shl(r1, cl).unwrap();

                            if instr.i_reg[0] != instr.o_reg.unwrap() {
                                asm.mov(r3, r1).unwrap();
                            }
                        },
                        _ => panic!("Unsupported flag provided for Sub Instruction")
                    }
                },
                Operation::And => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                        .unwrap(), 64).into();
                    let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                        .get(&instr.o_reg.unwrap())
                                                        .unwrap(), 64).into();
                    asm.and(r1, r2).unwrap();

                    if instr.i_reg[0] != instr.o_reg.unwrap() {
                        asm.mov(r3, r1).unwrap();
                    }
                },
                Operation::Xor => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                        .unwrap(), 64).into();
                    let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                        .get(&instr.o_reg.unwrap())
                                                        .unwrap(), 64).into();
                    asm.xor(r1, r2).unwrap();

                    if instr.i_reg[0] != instr.o_reg.unwrap() {
                        asm.mov(r3, r1).unwrap();
                    }
                },
                Operation::Or => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[1])
                                                        .unwrap(), 64).into();
                    let r3: AsmRegister64 = convert_reg(*reg_mapping
                                                        .get(&instr.o_reg.unwrap())
                                                        .unwrap(), 64).into();
                    asm.or(r1, r2).unwrap();

                    if instr.i_reg[0] != instr.o_reg.unwrap() {
                        asm.mov(r3, r1).unwrap();
                    }
                },
                Operation::Jmp(addr) => {
                    if let Some(label) = label_map.get(&addr) {
                        asm.jmp(*label).unwrap();
                    } else {
                        if let Some(jit_addr) = self.lookup(addr) {
                            asm.jmp(jit_addr as u64).unwrap();
                        } else {
                            let mut jit_exit = asm.create_label();
                            asm.mov(rcx, ptr(r15+24u64)).unwrap();
                            asm.mov(rcx, ptr(rcx+addr)).unwrap();
                            asm.test(rcx, rcx).unwrap();
                            asm.jz(jit_exit).unwrap();
                            asm.jmp(rcx).unwrap();

                            asm.set_label(&mut jit_exit).unwrap();
                            function_epilogue1!(1, addr, asm);
                        }
                    }
                },
                Operation::Ret => {
                    asm.ret().unwrap();
                }
                Operation::Call(addr) => {
                    if let Some(jit_addr) = self.lookup(addr) {
                        asm.call(jit_addr as u64).unwrap();
                    } else {
                        let mut jit_exit = asm.create_label();
                        let mut reentry  = asm.create_label();
                        asm.mov(rcx, ptr(r15 + 24u64)).unwrap();
                        asm.mov(rcx, ptr(rcx+addr)).unwrap();
                        asm.test(rcx, rcx).unwrap();
                        asm.jz(jit_exit).unwrap();
                        asm.call(rcx).unwrap();
                        asm.jmp(reentry).unwrap();

                        // TODO proper cleanup on jit-exit
                        asm.set_label(&mut jit_exit).unwrap();
                        function_epilogue1!(1, addr, asm);

                        asm.set_label(&mut reentry).unwrap();
                    }
                },
                Operation::CallReg => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                        .unwrap(), 64).into();

                    let mut jit_exit = asm.create_label();
                    let mut reentry  = asm.create_label();
                    asm.mov(rcx, ptr(r15 + 24u64)).unwrap();
                    asm.mov(rcx, ptr(rcx+r1)).unwrap();

                    asm.test(rcx, rcx).unwrap();
                    asm.jz(jit_exit).unwrap();
                    asm.call(rcx).unwrap();
                    asm.jmp(reentry).unwrap();

                    // TODO proper cleanup on jit-exit
                    asm.set_label(&mut jit_exit).unwrap();
                    function_epilogue2!(1, r1, asm);

                    asm.set_label(&mut reentry).unwrap();
                },
                Operation::Mov => {
                    let r1: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.o_reg.unwrap())
                                                        .unwrap(), 64).into();
                    let r2: AsmRegister64 = convert_reg(*reg_mapping.get(&instr.i_reg[0])
                                                                .unwrap(), 64).into();
                    asm.mov(r1, r2).unwrap();
                }
                _ => { panic!("unimplemented instr: {:?}", instr); }
            }
        }

        self.dump_instrs(&asm.instructions());

        // Actually compile the function and return the address it is compiled at
        Some(self.add_jitblock(&asm.assemble(0x0).unwrap(), init_pc))
    }

    /*      Handle Spill

       mov rax into reg_arr + rax_offset
       load spill_reg from reg_arr
        -> do stuff with it
       load spill_reg back into reg_arr
       load rax from reg_arr + rax_offset
    */

    fn dump_instrs(&self, instrs: &[ice_instr]) {
        let mut formatter = NasmFormatter::new();
        let mut output = String::new();

        for instr in instrs {
            output.clear();
            formatter.format(&instr, &mut output);
            println!("{:#?}", output);
        }
    }
}
