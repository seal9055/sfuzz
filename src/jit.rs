use std::sync::Mutex;
use std::sync::RwLock;

//use crate::irgraph::IRGraph;

use crate::ssa_builder::SSABuilder;
//use iced_x86::code_asm::*;

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

#[derive(Default, Debug)]
pub struct Statistics {
    pub total_cases: usize,
}

#[derive(Debug)]
pub struct Jit {
    pub jit_backing: Mutex<(&'static mut [u8], usize)>,

    pub lookup_arr: RwLock<Vec<usize>>,

    // TODO move stats out of here and into messages
    pub stats: Mutex<Statistics>,
}

impl Jit {
    pub fn new(address_space_size: usize) -> Self {
        Jit {
            jit_backing: Mutex::new((alloc_rwx(16*1024*1024), 0)),
            lookup_arr: RwLock::new(vec![0; address_space_size / 4]),
            stats: Mutex::new(Statistics::default()),
        }
    }

    // Probably gonna remove this
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
            None
        } else {
            Some(addr)
        }
    }

    /// Compile an IRGraph into x86 machine code
    pub fn compile(&self, _ssa_graph: &mut SSABuilder) -> Option<usize> {
        /*
        let mut asm: CodeAssembler;
        let label_map: BTreeMap<usize, Label> = BTreeMap::new();


        // TODO save state for vmexit macro

        for instr in irgraph.instrs {
            asm = CodeAssembler::new(64).unwrap();

            println!("{:x?}", instr);

            match instr.op {
                Operation::Loadi(v) => {
                    // let reg = get_reg();
                    // sign extend stuff
                    //asm.mov(reg, val).unwrap();
                    panic!("loadi hit");
                }

                _ => { panic!("unimplemented instr"); }
            }

        }
        */
        Some(10)
    }
}
