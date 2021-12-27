use crate::{
    emulator::{Fault},
};

/// The starting address for our memory allocator
const FIRSTALLOCATION: usize = 0x10000 - 0x8;

/// Used in this manner, the permissions can easily be used for bitflag permission checks
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
struct Perms;
impl Perms {
    pub const UNSET:   u8 = 0;
    pub const EXECUTE: u8 = 0x1;
    pub const WRITE:   u8 = 0x2;
    pub const READ:    u8 = 0x4;
    pub const ISALLOC: u8 = 0x8;
}

/// Describes the virtual memory space that each emulator uses (each emulator has their own)
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
    /// Allocate initial memory space
    pub fn new(size: usize) -> Self {
        Mmu {
            memory:       vec![0u8; size],
            permissions:  vec![0u8; size],
            alloc_addr:   FIRSTALLOCATION,
        }
    }

    /// Fork the mmu's memory
    pub fn fork(&self) -> Self {
        Mmu {
            memory:      self.memory.clone(),
            permissions: self.permissions.clone(),
            alloc_addr:  self.alloc_addr,
        }
    }

    /// Reset the mmu's memory
    pub fn reset(&mut self, other: &Mmu) {
        self.memory = other.memory.clone();
    }

    /// Set permissions at {addr} to {p} for {size} bytes
    fn set_permissions(&mut self, addr: usize, size: usize, p: u8) -> Option<()> {
        if size == 0 { return Some(()); }
        let end_addr = addr.checked_add(size)?;
        for i in addr..end_addr { self.permissions[i as usize] = p; }
        Some(())
    }

    /// Validate that a byte in memory has a certain permission
    fn check_perms(perm_to_check: u8, control: u8) -> Option<()> {
        if perm_to_check & control != control {
            return None;
        }
        Some(())
    }

    /// If perms::WRITE are set, write {size} bytes from {data} into the memory space at {addr}
    fn write_mem(&mut self, addr: usize, data: &[u8], size: usize) -> Result<(), Fault> {
        if size > data.len() { return Err(Fault::WriteFault(data.len())); }
        let end_addr = addr.checked_add(size).ok_or(Fault::IntegerOverflow)?;
        for i in addr..end_addr {
            Mmu::check_perms(self.permissions[i], Perms::WRITE).ok_or(Fault::WriteFault(i))?;
        }
        self.memory[addr..end_addr].copy_from_slice(&data[0..size]);
        Ok(())
    }

    /// If perms::READ are set, read {size} bytes from the memory space into the {data} reference
    fn read_mem(&mut self, addr: usize, data: &mut [u8], size: usize) -> Result<(), Fault> {
        if size > data.len() { return Err(Fault::ReadFault(data.len())); }
        let end_addr = addr.checked_add(size).ok_or(Fault::IntegerOverflow)?;
        for i in addr..end_addr {
            Mmu::check_perms(self.permissions[i], Perms::READ).ok_or(Fault::ReadFault(i))?;
        }
        data.copy_from_slice(&self.memory[addr..end_addr]);
        Ok(())
    }

    /// Load a given segment into memory
    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        // Set permissions to perms::WRITE to avoid errors during write_mem and perform the write
        self.set_permissions(segment.vaddr as usize, segment.memsz, Perms::WRITE)?;
        self.write_mem(segment.vaddr, data, segment.filesz as usize).ok()?;

        // ELF files can contain padding that needs to be loaded into memory but does not exist
        // in the file on disk, we still need to fill it up in memory though
        let padding = vec![0u8; (segment.memsz - segment.filesz) as usize];
        self.write_mem(segment.vaddr.checked_add(segment.filesz as usize)?, 
                       &padding, padding.len()).ok()?;

        // Set the permissions to the proper values as specified by the segment header information
        self.set_permissions(segment.vaddr, segment.memsz, segment.flags as u8)?;
        Some(())
    }
    
    /// Allocate some new RW memory, memory is never repeated, each allocation returns fresh memory,
    /// even if a prior allocation was free'd
    pub fn allocate(&mut self, size: usize) -> Option<usize> {
        // 0x10 byte align the allocation size and some additional bytes that can be used for an 
        // inlined size field.
        let aligned_size = (size + 0x18) & !0xf;
        let base = self.alloc_addr + 8;

        // Cannot allocate without running out of memory
        if base >= self.memory.len() || base.checked_add(aligned_size)? >= self.memory.len() { 
            return None; 
        }

        // Write sizefield into memory region 8 bytes prior to allocation (inline metadata)
        unsafe {
            *(((self.memory.as_ptr() as usize).checked_add(base)? - 8) as *mut usize) 
                = aligned_size; 
        };

        // Set Write permissions on allocated memory region and increase the next allocation addr
        self.set_permissions(base, size, Perms::WRITE | Perms::READ);
        self.alloc_addr = self.alloc_addr.checked_add(aligned_size)?;

        // Overwrite the size_field meta_data with special permission to indicate that it was
        // properly allocated using malloc. This allows us to check for invalid free's if the
        // permission is not set
        unsafe {
            *(((self.permissions.as_ptr() as usize).checked_add(base)? - 8) as *mut usize)
                = Perms::ISALLOC as usize;
        };

        Some(base)
    }

    /// Free a region of previously allocated memory
    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        if addr > self.memory.len() { return Err(Fault::InvalidFree(addr)); }

        // Retrieve sizefield that was stored as inlined metadata 8 bytes prior to the chunk
        let size = unsafe { *(((self.memory.as_ptr() as usize).checked_add(addr)
                               .ok_or(Fault::IntegerOverflow)? - 8) as *const usize) };

        // Verify that the permissions at the specified size field match up with a valid allocation
        unsafe {
            if *(((self.permissions.as_ptr() as usize).checked_add(addr)
                  .ok_or(Fault::IntegerOverflow)? - 8) as *const usize) != 
                Perms::ISALLOC as usize { return Err(Fault::InvalidFree(addr)); }
        };

        // Unset all permissions including metadata
        self.set_permissions(addr-8, size, Perms::UNSET);
        
        Ok(())
    }
}

/// Some simple unit tests to test individual functions of the mmu module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_valid_allocation() {
        let mut mem = Mmu::new(8 * 1024 * 1024);

        if mem.allocate(0x40).is_none() {
            panic!("Something went wrong during allocation");
        }
    }

    #[test]
    fn very_large_allocation() {
        let mut mem = Mmu::new(8 * 1024 * 1024);

        if mem.allocate(8 * 1024 * 1024).is_some() {
            panic!("Should have errored out due to large size");
        }
    }

    #[test]
    fn multiple_allocations() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut addrs = Vec::new();
        let mut i = 0;

        while let Some(x) = mem.allocate(0x40) {
            if i >= 5 { break; }
            i += 1;
            addrs.push(x);
        }
        assert_eq!(addrs.len(), 5);
    }

    #[test]
    fn zero_allocation() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr1: usize = 0;
        let mut _addr2: usize = 0;

        if let Some(x) = mem.allocate(0) {
            _addr1 = x;
        } else {
            panic!("Size of zero should still return minimum allocation");
        }
        if let Some(x) = mem.allocate(0) {
            _addr2 = x;
        } else {
            panic!("Size of zero should still return minimum allocation");
        }

        // Can't easily check the size, but we can make sure that the second 0 allocation
        // is allocated at a higher address than the first
        assert!(_addr1 < _addr2);
    }

    #[test]
    fn normal_valid_allocation_check_perms() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr: usize = 0;

        if let Some(x) = mem.allocate(0x40) {
            _addr = x;
        } else {
            panic!("Something went wrong during allocation");
        }
        assert!(Mmu::check_perms(mem.permissions[_addr+0x20], Perms::WRITE).is_some());
    }

    #[test]
    fn normal_valid_free() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr: usize = 0;

        if let Some(x) = mem.allocate(0x40) {
            _addr = x;
        } else {
            panic!("failure during allocation");
        }

        if let Err(e) = mem.free(_addr) {
            panic!("unexpected failure during first free: {:?}", e);
        }
    }

    #[test]
    fn free_on_memory_not_allocated_by_alloc() {
        let mut mem = Mmu::new(1024 * 1024);
        if let Err(v) = mem.free(1024) {
            match v {
                Fault::InvalidFree(_) => {},
                _ => { panic!("Free threw the wrong type of error"); }
            }
        } else { panic!("Free did not throw any error at all"); }
    }
    
    #[test]
    fn double_free() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr: usize = 0;

        if let Some(x) = mem.allocate(0x40) {
            _addr = x;
        } else {
            panic!("failure during allocation");
        }

        if let Err(e) = mem.free(_addr) {
            panic!("unexpected failure during first free: {:?}", e);
        }

        if let Ok(()) = mem.free(_addr) {
            panic!("Second free on same memory should have given an error");
        }
    }

    #[test]
    fn multiple_frees() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut addrs = Vec::new();
        let mut i = 0;

        while let Some(x) = mem.allocate(0x40) {
            if i > 5 { break; }
            i += 1;
            addrs.push(x);
        }

        while let Some(x) = addrs.pop() {
            if let Err(e) = mem.free(x) {
                panic!("unexpected failure during one of the free's: {:?}", e);
            }
        }
    }

    #[test]
    fn normal_valid_segmentloader() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x5,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x100,
            memsz:    0x100,
            align:    0x1000,
        };
        let data = vec![0x41u8; 0x200];
        let len = 0x20;
        let mut read_buf = vec![0; len];

        if mem.load_segment(seg, &data).is_none() { panic!("Error during initial load"); }

        if let Err(e) = mem.read_mem(0x400000, &mut read_buf, 0x20) { 
            panic!("Error while attempting to read memory with fault: {:?}", e); 
        }
        
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));
    }

    #[test]
    fn load_multiple_segments() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg1 = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x5,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x100,
            memsz:    0x100,
            align:    0x1000,
        };
        let seg2 = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x5,
            offset:   0x0,
            vaddr:    0x410000,
            paddr:    0x410000,
            filesz:   0x100,
            memsz:    0x100,
            align:    0x1000,
        };
        let data1 = vec![0x41u8; 0x200];
        let data2 = vec![0x42u8; 0x200];
        let len = 0x20;
        let mut read_buf = vec![0; len];

        if mem.load_segment(seg1, &data1).is_none() { panic!("Error during seg1 load"); }
        if mem.load_segment(seg2, &data2).is_none() { panic!("Error during seg2 load"); }

        if let Err(e) = mem.read_mem(0x400000, &mut read_buf, 0x20) {
            panic!("Error while attempting to read memory from seg1 with fault: {:?}", e); 
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));

        if let Err(e) = mem.read_mem(0x410000, &mut read_buf, 0x20) {
            panic!("Error while attempting to read memory from seg2 with fault: {:?}", e); 
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x42));
    }

    #[test]
    fn valid_write_mem() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x2,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x200,
            memsz:    0x200,
            align:    0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let write_data = vec![0x42u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() { panic!("Error during seg load"); }
        
        if let Err(e) = mem.write_mem(0x400000, &write_data, 0x20) {
            panic!("Error occured while writing memory: {:?}", e);
        }
    }

    #[test]
    fn write_to_nonwriteable_memory() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x1,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x200,
            memsz:    0x200,
            align:    0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let write_data = vec![0x42u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() { panic!("Error during seg load"); }
        
        if let Ok(_) = mem.write_mem(0x400000, &write_data, 0x20) {
            panic!("write_mem did not properly return an error on invalid write");
        }
    }

    #[test]
    fn valid_read_mem() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x4,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x200,
            memsz:    0x200,
            align:    0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let mut read_buf = vec![0x0u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() { panic!("Error during seg load"); }
        
        if let Err(e) = mem.read_mem(0x400000, &mut read_buf, 0x20) {
            panic!("Error occured while reading memory: {:?}", e);
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));
    }

    #[test]
    fn read_from_nonreadable_memory() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags:    0x2,
            offset:   0x0,
            vaddr:    0x400000,
            paddr:    0x400000,
            filesz:   0x200,
            memsz:    0x200,
            align:    0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let mut read_buf = vec![0x21u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() { panic!("Error during seg load"); }
        
        if let Ok(_) = mem.read_mem(0x400000, &mut read_buf, 0x20) {
            panic!("read_mem did not properly return an error on invalid write");
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x21));
    }
}
