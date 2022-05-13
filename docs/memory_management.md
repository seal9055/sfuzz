# Memory Management

#### Overview

The memory management unit is responsible for simulating a memory environment for the running 
target. Each spawned emulator gets its mmu instance and memory space. The main components of 
this mmu are 2 continuous blocks of memory (one for the actual memory and one for permissions), 
and an api that exposes various operations on this memory such as allocations, frees, reads and 
writes. The permissions region is necessary because this emulator makes use of byte-level permission 
checks. These are done by maintaining an entire memory space used purely for permissions. The mmu 
exposes a custom alloc/free implementation that in combination with the byte level memory 
permissions detect most memory corruption bugs in a similar manner to address sanitizer.

Most of the code pertaining to these features can be found in [mmu.rs](https://github.com/seal9055/sfuzz/blob/main/src/mmu.rs) More detailed descriptions of some of these features are provided below.

#### Byte Level Permission Checks
On most architectures, permissions are handled at the hardware level. This means that whenever an
instruction tries to access memory, with permissions not matching the access pattern, an abort is
generated which is then handled at the software level. Generally these permissions are handled at
the page table level so each page of memory has its own permission bits. With guard pages placed
around critical locations such as kernel memory, this protects the operating system from crucial
many bugs, and prevents memory accesses that are completely off of their target. When it comes to
exploitation however, a small 8 byte out of bounds access can oftentimes already be enough to
compromise the security of an application.  

A tool commonly used while fuzzing is address sanitizer (also referred to as asan). When a binary is 
compiled using asan, it is instrumented at compile time with extra checks that make sure that every 
memory access has the correct access permissions. This tool however has a few very relevant issues. 
For one it requires access to the binaries source code to recompile it with proper instrumentation. 
This makes it only useful to open source projects, which especially when fuzzing embedded systems,
is often not available. Secondly, asan has a very non-significant performance overhead. According to
a study conducted by google in 2012 (AddressSanitizer: A Fast Address Sanity Checker), it resulted
in a 73% slowdown, which is quite a bit, especially when operating a fuzzer which is entirely
dependant on its performance. This slowdown however was worth it due to the power of byte level
permission checks and led to 300 new bugs being discovered in the Chrome browser at the time.

In this case since the binary is being run in a custom JIT compiler, both of these drawbacks can be 
almost entirely mitigated. Not having source code available is not an issue at all anymore since all 
of the code is being generated at runtime anyways. As for the performance aspects, EXECUTE
permissions are almost entirely free since they are checked once when a function is first compiled,
and then assumed to be true for the rest of program execution. This would need some changes when
dealing with JIT compilers that frequently change their executable memory mappings, but for 99% of
use cases it should suffice. As for load and store instructions (that require the READ and WRITE
permissions), the checks consist of 5 assembly instructions (1 memory load, 1 conditional jmp and 3
arithmetic instructions). While this results in some additional overhead when performing frequent
memory accesses, it is nowhere near as expensive as address sanitizer.

These permission bits mean that every out of bounds memory access (even if it is just a
single byte) instantly results in a notification to the fuzzer which can then modify its corpus to
focus on this bug and attempt to increase the out of bounds bug. This permission model also applies
to library functions such as malloc & free. These are hooked at compile time to instead call custom
malloc/free implementations that support this byte level memory model. These hooked functions also
include additional checks to completely destruct free'd memory so common heap bugs such as use
after free's or double free's are instantly reported as well instead of leading to undefined
behavior.

#### Dirty-bit Memory Resets
In the current implementation each new address space is 64mb large. This means that on each new fuzz
case, this entire space needs to be reset to its initial state. Doing a massive 64mb memcpy() on
each new fuzz case is very impractical and leads to completely unacceptable performance. Here we can
borrow a concept that is common in the operating systems world: dirty bits. In operating systems,
these are mainted at the page table level similar to the permissions. This bit is set whenever a
write to memory occurs. This means that when copying memory between different cache levels, or just
clearing memory, the page table can be traversed, and only pages with the dirty bit set need to have
work done on them.

The same principal applies to this fuzzer. When a fuzzer is run, only a very small percentage of
this 64mb address space is actually overwritten. This means that by maintaining a dirty bit list,
we can selectively chose which pages are reset while leaving most of the memory intact. The memory
space in this case is not maintained in a page table so some of the implementation details differ,
but the principal remains.

The implementation in this project was heavily influenced by Brandon Falk's prior research into
obtaining extremely fast memory resets. He tested multiple different approaches over the years, but
eventually settled on one similar to this since walking the page tables in the jit-compiled assembly
code to set a dirty bit was unnecessarily expensive. Instead 2 vector data structures are
maitained. Whenever memory is dirtied, the address is pushed to an initially empty array that
contains a listing of all dirtied memory regions. Additionally a dirty bitmap is maintained that is
used to verify that only 1 address from each page is pushed to this array to avoid duplicates.
Populating this vector during execution is very simple and only requires 6 additional instructions
during store operations. While resetting, the fuzzer can then just iterate through the previously 
populator vector and free the address ranges that were pushed to the vector.

#### Virtualized Files


#### Future Work
For RISC-V the current memory/permission model is totally sufficient, but if this fuzzer were to be
used against x86_64 for example, issues would quickly come up. X86 uses a much larger memory space/area,
so simply loading the entire space into memory is inpractical and will cause many cache-related 
performance slowdowns. In that light, I would like to eventually implement a page-table structure to only
map in pages that are actually used to more easily support larger memory spaces.
