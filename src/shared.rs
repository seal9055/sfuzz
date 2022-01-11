use std::sync::Mutex;
use std::sync::RwLock;

#[cfg(target_os="linux")]
pub fn alloc_rwx(size: usize) -> &'static mut [u8] {
    extern {
        fn mmap(addr: *mut u8, length: usize, prot: i32, flags: i32, fd: i32,
                offset: usize) -> *mut u8;
    }

    unsafe {
        // Alloc RWX and MAP_PRIVATE | MAP_ANON
        let ret = mmap(0 as *mut u8, size, 7, 34, -1, 0);
        assert!(!ret.is_null());

        std::slice::from_raw_parts_mut(ret, size)
    }
}

#[derive(Debug)]
pub struct Shared {
    pub jit: Mutex<(&'static mut [u8], usize)>,

    pub lookup_arr: RwLock<Vec<usize>>,
}

impl Shared {
    pub fn new(address_space_size: usize) -> Self {
        Shared {
            jit: Mutex::new((alloc_rwx(16*1024*1024), 0)),
            lookup_arr: RwLock::new(vec![0; address_space_size / 4]),
        }
    }

    pub fn add_jitblock(&self, code: &[u8], pc: usize) -> usize {
        let mut jit = self.jit.lock().unwrap();

        let jit_inuse = jit.1;
        jit.0[jit_inuse..jit_inuse + code.len()].copy_from_slice(code);

        let addr = jit.0.as_ptr() as usize + jit_inuse;

        // add mapping
        self.lookup_arr.write().unwrap()[pc] = addr;

        jit.1 += code.len();

        // Return the JIT address of the code we just compiled
        addr
    }

    pub fn lookup(&self, pc: usize) -> Option<usize> {
        let addr = self.lookup_arr.read().unwrap()[pc];
        if addr == 0 {
            return None;
        } else {
            return Some(addr);
        }
    }
}
