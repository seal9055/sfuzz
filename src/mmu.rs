use crate::emulator::Fault;

/// The starting address for our memory allocator
const FIRSTALLOCATION: usize = 0x60000 - 0x8;

/// Used in this manner, the permissions can easily be used for bitflag permission checks
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct Perms;
impl Perms {
    pub const UNSET: u8 = 0;
    pub const EXECUTE: u8 = 0x1;
    pub const WRITE: u8 = 0x2;
    pub const READ: u8 = 0x4;
    pub const ISALLOC: u8 = 0x8;
}

/// Trait + Corresponding macro allows reading/writing values from a Vec<u8> using a generic type T
pub trait ByteConversions {
    fn write_to(self, dst: &mut [u8]);
    fn read_from(src: &[u8]) -> Self;
}

macro_rules! impl_byte_conversions {
    ($($t:ident),*) => { $(
        impl ByteConversions for $t {
            fn write_to(self, dst: &mut [u8]) {
                let array = self.to_ne_bytes();
                dst.copy_from_slice(&array);
            }
            fn read_from(src: &[u8]) -> Self {
                let mut array = [0; std::mem::size_of::<Self>()];
                array.copy_from_slice(src);
                Self::from_ne_bytes(array)
            }
        }
    )* }
}
impl_byte_conversions!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128);

/// Describes the virtual memory space that each emulator uses (each emulator has their own)
#[derive(Debug)]
pub struct Mmu {
    /// Block of memory used by an emulator instance, contains the actual memory
    pub memory: Vec<u8>,

    /// Memory permissions used by an emulator instance. Map 1:1 to memory
    pub permissions: Vec<u8>,

    /// Holds the current program break at which new memory is allocated whenever needed
    alloc_addr: usize,

    pub dirty: Vec<usize>,

    pub dirty_bitmap: Vec<u64>,

    pub dirty_size: u64,
}

impl Mmu {
    /// Allocate initial memory space
    pub fn new(size: usize) -> Self {
        Mmu {
            memory:       vec![0u8; size],
            permissions:  vec![0u8; size],
            alloc_addr:   FIRSTALLOCATION,
            dirty:        Vec::with_capacity(size / 4096 + 1),
            dirty_bitmap: vec![0u64; size / 4096 / 64 + 1],
            dirty_size:   0,
        }
    }

    /// Fork the mmu's memory so another emulator can be started with it. This will be an exact copy
    /// of the input emulator, except that the dirty lists will be emptied.
    pub fn fork(&self) -> Self {
        let size = self.memory.len();
        Mmu {
            memory:       self.memory.clone(),
            permissions:  self.permissions.clone(),
            alloc_addr:   self.alloc_addr,
            dirty:        Vec::with_capacity(size / 4096 + 1),
            dirty_bitmap: vec![0u64; size / 4096 / 64 + 1],
            dirty_size:   0,
        }
    }

    /// Restores memory back to original state prior to any fuzz cases
    pub fn reset(&mut self, other: &Mmu) {
        for &block in &self.dirty {
            let start = block * 4096;
            let end   = (block + 1) * 4096;

            // Completely reset the high level bitmap
            self.dirty_bitmap[block / 64] = 0;

            // Reset all dirtied memory pages
            self.memory[start..end].copy_from_slice(&other.memory[start..end]);
            self.permissions[start..end].copy_from_slice(&other.permissions[start..end]);
        }
        // Reset dirty list
        self.dirty.clear();
        self.dirty_size = 0;

        // Reset current base address of heap allocator
        self.alloc_addr = other.alloc_addr;
    }

    /// Set permissions at {addr} to {p} for {size} bytes
    fn set_permissions(&mut self, addr: usize, size: usize, p: u8) -> Option<()> {
        if size == 0 {
            return Some(());
        }
        let end_addr = addr.checked_add(size)?;
        for i in addr..end_addr {
            self.permissions[i as usize] = p;
        }
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
    pub fn write_mem(&mut self, addr: usize, data: &[u8], size: usize) -> Result<(), Fault> {
        if size > data.len() {
            return Err(Fault::WriteFault(data.len()));
        }
        let end_addr = addr.checked_add(size).ok_or(Fault::IntegerOverflow)?;
        for i in addr..end_addr {
            Mmu::check_perms(self.permissions[i], Perms::WRITE).ok_or(Fault::WriteFault(i))?;
        }
        self.memory[addr..end_addr].copy_from_slice(&data[0..size]);

        let block_start = addr / 4096;
        let block_end   = (addr + size) / 4096;
        for block in block_start..=block_end {
            let idx = block / 64;
            let bit = block % 64;

            // If the bitmap does not already have an entry for the current write
            if self.dirty_bitmap[idx] & (1 << bit) == 0 {
                // Add a new entry to the dirty list
                self.dirty.push(block);

                // Update the dirty bitmap so that this page is not marked as dirty again on further
                // writes
                self.dirty_bitmap[idx] |= 1 << bit;
            }
        }
        Ok(())
    }

    /// If all permissions are set, read {size} bytes from the memory space into the {data}
    /// reference
    pub fn read_into(&mut self, addr: usize, data: &mut [u8], size: usize, perms: u8)
            -> Result<(), Fault> {
        if size > data.len() {
            return Err(Fault::ReadFault(data.len()));
        }
        let end_addr = addr.checked_add(size).ok_or(Fault::IntegerOverflow)?;
        for i in addr..end_addr {
            Mmu::check_perms(self.permissions[i], perms).ok_or(Fault::ReadFault(i))?;
        }
        data.copy_from_slice(&self.memory[addr..end_addr]);
        Ok(())
    }

    /// If all permissions are set, read size_of<T> bytes from the memory space and return them as
    /// type T. Uses generics to support reads of different sizes (u8, u16, ...). The return type
    /// is specified by the caller of the function
    pub fn read_at<T: ByteConversions>(&self, addr: usize, perms: u8) -> Result<T, Fault> {
        for i in addr..addr.checked_add(std::mem::size_of::<T>()).ok_or(Fault::IntegerOverflow)? {
            Mmu::check_perms(self.permissions[i], perms).ok_or(Fault::ReadFault(i))?;
        }
        Ok(T::read_from(&self.memory[addr..addr.checked_add(std::mem::size_of::<T>())
                        .ok_or(Fault::IntegerOverflow)?]))
    }

    /// Load a given segment into memory
    pub fn load_segment(&mut self, segment: elfparser::ProgramHeader, data: &[u8]) -> Option<()> {
        // Set permissions to perms::WRITE to avoid errors during write_mem and perform the write
        self.set_permissions(segment.vaddr as usize, segment.memsz, Perms::WRITE)?;
        self.write_mem(segment.vaddr, data, segment.filesz as usize).ok()?;

        // ELF files can contain padding that needs to be loaded into memory but does not exist
        // in the file on disk, we still need to fill it up in memory though
        let padding = vec![0u8; (segment.memsz - segment.filesz) as usize];
        self.write_mem(segment.vaddr.checked_add(segment.filesz as usize)?, &padding,
                       padding.len()).ok()?;

        // Set the permissions to the proper values as specified by the segment header information
        self.set_permissions(segment.vaddr, segment.memsz, segment.flags as u8)?;
        Some(())
    }

    /// Allocate some new RW memory, memory is never repeated, each allocation returns fresh memory,
    /// even if a prior allocation was free'd
    pub fn allocate(&mut self, size: usize, perms: u8) -> Option<usize> {
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
            *(((self.memory.as_ptr() as usize).checked_add(base)? - 8) as *mut usize) =
                aligned_size;
        };

        // Set Write permissions on allocated memory region and increase the next allocation addr
        self.set_permissions(base, size, perms);
        self.alloc_addr = self.alloc_addr.checked_add(aligned_size)?;

        // Overwrite the size_field meta_data with special permission to indicate that it was
        // properly allocated using malloc. This allows us to check for invalid free's if the
        // permission is not set
        unsafe {
            *(((self.permissions.as_ptr() as usize).checked_add(base)? - 8) as *mut usize) =
                Perms::ISALLOC as usize;
        };

        Some(base)
    }

    /// Free a region of previously allocated memory
    pub fn free(&mut self, addr: usize) -> Result<(), Fault> {
        if addr > self.memory.len() {
            return Err(Fault::InvalidFree(addr));
        }

        // Retrieve sizefield that was stored as inlined metadata 8 bytes prior to the chunk
        let size = unsafe {*(((self.memory.as_ptr() as usize).checked_add(addr)
               .ok_or(Fault::IntegerOverflow)? - 8) as *const usize) };

        // Verify that the permissions at the specified size field match up with a valid allocation
        unsafe {
            if *(((self.permissions.as_ptr() as usize).checked_add(addr)
                  .ok_or(Fault::IntegerOverflow)? - 8) as *const usize) != Perms::ISALLOC as usize {
                return Err(Fault::InvalidFree(addr));
            }
        };

        // Unset all permissions including metadata
        self.set_permissions(addr - 8, size, Perms::UNSET);

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

        if mem.allocate(0x40, Perms::READ | Perms::WRITE).is_none() {
            panic!("Something went wrong during allocation");
        }
    }

    #[test]
    fn very_large_allocation() {
        let mut mem = Mmu::new(8 * 1024 * 1024);

        if mem.allocate(8 * 1024 * 1024, Perms::READ | Perms::WRITE).is_some() {
            panic!("Should have errored out due to large size");
        }
    }

    #[test]
    fn multiple_allocations() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut addrs = Vec::new();
        let mut i = 0;

        while let Some(x) = mem.allocate(0x40, Perms::READ | Perms::WRITE) {
            if i >= 5 {
                break;
            }
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

        if let Some(x) = mem.allocate(0, Perms::READ | Perms::WRITE) {
            _addr1 = x;
        } else {
            panic!("Size of zero should still return minimum allocation");
        }
        if let Some(x) = mem.allocate(0, Perms::READ | Perms::WRITE) {
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

        if let Some(x) = mem.allocate(0x40, Perms::READ | Perms::WRITE) {
            _addr = x;
        } else {
            panic!("Something went wrong during allocation");
        }
        assert!(Mmu::check_perms(mem.permissions[_addr + 0x20], Perms::WRITE).is_some());
    }

    #[test]
    fn normal_valid_free() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr: usize = 0;

        if let Some(x) = mem.allocate(0x40, Perms::READ | Perms::WRITE) {
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
                Fault::InvalidFree(_) => {}
                _ => {
                    panic!("Free threw the wrong type of error");
                }
            }
        } else {
            panic!("Free did not throw any error at all");
        }
    }

    #[test]
    fn double_free() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let mut _addr: usize = 0;

        if let Some(x) = mem.allocate(0x40, Perms::READ | Perms::WRITE) {
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

        while let Some(x) = mem.allocate(0x40, Perms::READ | Perms::WRITE) {
            if i > 5 {
                break;
            }
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
            flags: 0x5,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x100,
            memsz: 0x100,
            align: 0x1000,
        };
        let data = vec![0x41u8; 0x200];
        let len = 0x20;
        let mut read_buf = vec![0; len];

        if mem.load_segment(seg, &data).is_none() {
            panic!("Error during initial load");
        }

        if let Err(e) = mem.read_into(0x400000, &mut read_buf, 0x20, Perms::READ) {
            panic!("Error while attempting to read memory with fault: {:?}", e);
        }

        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));
    }

    #[test]
    fn load_multiple_segments() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg1 = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x5,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x100,
            memsz: 0x100,
            align: 0x1000,
        };
        let seg2 = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x5,
            offset: 0x0,
            vaddr: 0x410000,
            paddr: 0x410000,
            filesz: 0x100,
            memsz: 0x100,
            align: 0x1000,
        };
        let data1 = vec![0x41u8; 0x200];
        let data2 = vec![0x42u8; 0x200];
        let len = 0x20;
        let mut read_buf = vec![0; len];

        if mem.load_segment(seg1, &data1).is_none() {
            panic!("Error during seg1 load");
        }
        if mem.load_segment(seg2, &data2).is_none() {
            panic!("Error during seg2 load");
        }

        if let Err(e) = mem.read_into(0x400000, &mut read_buf, 0x20, Perms::READ) {
            panic!(
                "Error while attempting to read memory from seg1 with fault: {:?}",
                e
            );
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));

        if let Err(e) = mem.read_into(0x410000, &mut read_buf, 0x20, Perms::READ) {
            panic!(
                "Error while attempting to read memory from seg2 with fault: {:?}",
                e
            );
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x42));
    }

    #[test]
    fn valid_write_mem() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x2,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x200,
            memsz: 0x200,
            align: 0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let write_data = vec![0x42u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() {
            panic!("Error during seg load");
        }

        if let Err(e) = mem.write_mem(0x400000, &write_data, 0x20) {
            panic!("Error occured while writing memory: {:?}", e);
        }
    }

    #[test]
    fn write_to_nonwriteable_memory() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x1,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x200,
            memsz: 0x200,
            align: 0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let write_data = vec![0x42u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() {
            panic!("Error during seg load");
        }

        if let Ok(_) = mem.write_mem(0x400000, &write_data, 0x20) {
            panic!("write_mem did not properly return an error on invalid write");
        }
    }

    #[test]
    fn valid_read_mem() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x4,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x200,
            memsz: 0x200,
            align: 0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let mut read_buf = vec![0x0u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() {
            panic!("Error during seg load");
        }

        if let Err(e) = mem.read_into(0x400000, &mut read_buf, 0x20, Perms::READ) {
            panic!("Error occured while reading memory: {:?}", e);
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x41));
    }

    #[test]
    fn read_from_nonreadable_memory() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let seg = elfparser::ProgramHeader {
            seg_type: 0x1,
            flags: 0x2,
            offset: 0x0,
            vaddr: 0x400000,
            paddr: 0x400000,
            filesz: 0x200,
            memsz: 0x200,
            align: 0x1000,
        };
        let load_data = vec![0x41u8; 0x200];
        let mut read_buf = vec![0x21u8; 0x20];
        if mem.load_segment(seg, &load_data).is_none() {
            panic!("Error during seg load");
        }

        if let Ok(_) = mem.read_into(0x400000, &mut read_buf, 0x20, Perms::READ) {
            panic!("read_mem did not properly return an error on invalid write");
        }
        read_buf.iter().for_each(|e| assert_eq!(*e, 0x21));
    }

    #[test]
    fn valid_read_at() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let load_data = vec![0x1u8; 0x200];
        let addr = mem.allocate(0x100, Perms::READ | Perms::WRITE).unwrap();

        mem.write_mem(addr, &load_data, 0x50).unwrap();

        let v1: u8  = mem.read_at(addr, Perms::READ).unwrap();
        let v2: u16 = mem.read_at(addr, Perms::READ).unwrap();
        let v3: u32 = mem.read_at(addr, Perms::READ).unwrap();
        let v4: u64 = mem.read_at(addr, Perms::READ).unwrap();

        assert_eq!(v1, 0x1);
        assert_eq!(v2, 0x0101);
        assert_eq!(v3, 0x01010101);
        assert_eq!(v4, 0x0101010101010101);
    }

    #[test]
    #[should_panic]
    fn invalid_perms_read_at() {
        let mut mem = Mmu::new(8 * 1024 * 1024);
        let load_data = vec![0x1u8; 0x200];
        let addr = mem.allocate(0x100, Perms::READ | Perms::WRITE).unwrap();

        mem.write_mem(addr, &load_data, 0x50).unwrap();
        let _: u8  = mem.read_at(addr, Perms::EXECUTE).unwrap();
        let _: u32 = mem.read_at(addr, Perms::EXECUTE).unwrap();
    }
}
