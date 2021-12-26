use crate::ProgramHeader;
use byteorder::{LittleEndian, BigEndian, WriteBytesExt, ReadBytesExt};
use std::io::Cursor;

const FIRSTALLOCATION: usize = 0x10000 - 0x8;

#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
struct perms;
impl perms {
    pub const UNSET:   u8 = 0;
    pub const EXECUTE: u8 = 0x1;
    pub const WRITE:   u8 = 0x2;
    pub const READ:    u8 = 0x4;
    pub const ISALLOC: u8 = 0x8;
}

#[derive(Debug, Clone)]
pub struct Mmu {
    /// Block of memory used by an emulator instance, contains the actual memory
    memory: Vec<u8>, 

    /// Memory permissions used by an emulator instance. Map 1:1 to memory
    permissions: Vec<u8>, 

    /// Holds the current program break at which new memory is allocated whenever needed
    alloc_addr: usize,
}

impl Mmu {
    pub fn new(size: usize) -> Self {
        Mmu {
            memory:       vec![0u8; size],
            permissions:  vec![0u8; size],
            alloc_addr:   FIRSTALLOCATION,
        }
    }

    pub fn fork(&self) -> Self {
        Mmu {
            memory:      self.memory.clone(),
            permissions: self.permissions.clone(),
            alloc_addr:  self.alloc_addr,
        }
    }

    pub fn reset(&mut self, other: &Mmu) {
        self.memory = other.memory.clone();
    }

    pub fn set_permissions(&mut self, addr: usize, size: usize, p: u8) -> Option<()> {
        if size == 0 { return Some(()); }
        let end_addr = addr.checked_add(size)?;
        for i in addr..end_addr { self.permissions[i as usize] = p; }
        Some(())
    }

    fn check_perms(perm_to_check: u8, control: u8) -> Option<()> {
        if perm_to_check & control != control {
            return None;
        }
        Some(())
    }

    pub fn write_mem(&mut self, addr: usize, data: &[u8], size: usize) -> Option<()> {
        let end_addr = addr.checked_add(size)?;
        for i in addr..end_addr {
            Mmu::check_perms(self.permissions[i], perms::WRITE)
                .expect("Error on permission check in mmu.write_mem");
        }

        for i in 0..size {
            self.memory[addr + i] = data[i];
        }
        Some(())
    }

    pub fn load_mem(&mut self, section: ProgramHeader, data: &[u8]) {
        self.set_permissions(section.vaddr as usize, section.memsz, perms::WRITE);

        self.write_mem(section.vaddr, data, section.filesz as usize);

        let padding = vec![0u8; (section.memsz - section.filesz) as usize];
        self.write_mem(section.vaddr+section.filesz as usize, &padding, padding.len());

        self.set_permissions(section.vaddr, section.memsz, section.flags as u8);
    }
    
    /// Allocate some new RW memory, memory is never repeated, each allocation returns fresh memory,
    /// even if a prior allocation was free'd
    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        // 0x10 byte align the allocation size and some additional bytes that can be used for an 
        // inlined size field.
        let aligned_size = (size + 0x18) & !0xf;
        let base = self.alloc_addr + 8;

        // Cannot allocate without running out of memory
        if base >= self.memory.len() || (base + aligned_size) >= self.memory.len() { return None; }

        // Write sizefield into memory region 8 bytes prior to allocation (inline metadata)
        unsafe {
            *(((self.memory.as_ptr() as usize) + base - 8) as *mut usize) = aligned_size;
        };

        // Set Write permissions on allocated memory region and increase the next allocation addr
        self.set_permissions(base, size, perms::WRITE | perms::READ);
        self.alloc_addr = self.alloc_addr.checked_add(aligned_size)?;

        // Overwrite the size_field meta_data with special permission to indicate that it was
        // properly allocated using malloc. This allows us to check for invalid free's if the
        // permission is not set
        unsafe {
            *(((self.permissions.as_ptr() as usize) + base - 8) as *mut usize) 
                = perms::ISALLOC as usize;
        };

        println!("Allocated 0x{:x} bytes at 0x{:x} for a call with size 0x{:x}", aligned_size, base, size);

        Some(base)
    }

    /// Free a region of previously allocated memory
    pub fn free(&mut self, addr: usize) -> Option<()> {

        // Retrieve sizefield that was stored as inlined metadata 8 bytes prior to the chunk
        let size = unsafe { *(((self.memory.as_ptr() as usize) + addr - 8) as *const usize) };

        if addr > self.memory.len() { return None; }

        // Verify that the permissions at the specified size field match up with a valid allocation
        unsafe {
            if *(((self.permissions.as_ptr() as usize) + addr - 8) as *const usize) != 
                perms::ISALLOC as usize { return None; }
        };

        // Unset all permissions including metadata
        self.set_permissions(addr-8, size, perms::UNSET);
        
        println!("Free'd 0x{:x} bytes at 0x{:x}", size, addr);

        Some(())
    }
}
