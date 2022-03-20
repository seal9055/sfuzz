# sfuzz
Start date: Dec, 2021

JIT compiler / Fuzzer in development
Requires rustup nightly channel to compile

#### Overview

This project focuses on high performance RISC-V to x86\_64 binary translations for the purpose of
fuzzing.
<br>

**Task List**
- [X] RISCV instruction decoding
- [X] ELF parser to map target process into emulator memory
- [X] Basic memory management unit layout
- [X] Emulator outline that enables starting threads and intercepting syscalls
- [X] Lift code into intermediate representation
- [X] Convert intermediate representation code to ssa form
- [X] Register allocation
- [ ] JIT Compile into x86\_64 machine code
- [ ] Implement a good amount of commonly used syscalls in userland
- [ ] MMU improvements (dirty bit memory resets, Read-on-Write memory protection, etc)
- [ ] Codegen improvements (relevant compiler optimizations)
- [ ] Start fuzzing :tada:

<br>
The objective of this project is to highlight the benefits of using an emulated environment for
fuzzing. Many previous projects exist on this topic, but they almost exclusively use the qemu
emulation engine for the underlying emulation. While this engine does have a fairly mature
just-in-time compiler, it is not meant for fuzzing. During fuzzing, we intend to run the same
process thousands of times per second. This makes room for specialized optimizations that qemu does
not make strong use of, such as reusing the same memory space for each process run and only
resetting a limited amount of memory via dirty bit mechanics.<br><br>

Sfuzz starts by allocating a memory space for the main emulator and creating a JIT backing. Next
a binary is loaded from disk and parsed to map the relevant regions into the emulators address
space. Finally a stack is setup for the emulator, and function hooks at malloc & free are
registered so calls to these functions make use the custom heap functions of the mmu. Now
multiple copies of the emulator are created, and each one is started in a separate thread. These
threads will run until the fuzzer is shut off. The main thread is the dedicated thread to
calculate and provide statistics.

At this point each emulator starts doing it's own thing. The only shared information remaining
is the JIT backing containing compiled x86\_64 machine code and statistics that are sent to
the main
thread in batches. The emulator can now start running code. If a function has already been
compiled, the emulator jumps straight into the JIT buffer and starts executing instructions. If it
has not yet been compiled, the emulator thread that first runs into the code invokes the code
generation phase. This phase locks the JIT backing mutex until the code is compiled, thus stopping
all other threads.

The code generation procedure lifts all RISCV instructions into an intermediate
representation. This representation is converted to single static assignment form to accomodate
potential future optimizations. Finally x86\_64 register allocation is done and the code
is compiled into the JIT backing. Since this is a JIT compiler, the code generation needs to add
instructions that leave the vm on certain conditions. These include jumps to non-compiled code,
syscalls, hooked functions or various errors.

During execution, if the vm is left, the emulator determines the next step based on the exit code.
If the reason was a syscall or a hooked function, a callback function is invoked to handle this
event. If new code needs to be compiled, the above process just repeats.

#### Riscv toolchain to test the binary

This sets up a toolchain to compile riscv binaries that can be loaded/used by this project.
```
git clone https://github.com/riscv-collab/riscv-gnu-toolchain && cd riscv-gnu-toolchain
./configure --prefix=/opt/riscv --with-arch=rv64i --enable-multilib
sudo make linux -j 8
```

#### Memory Management

The memory management unit is responsible for simulating a memory environment. Each spawned
emulator gets its mmu. The main components of this mmu are 2 continuous blocks of memory (one
for the actual memory and one for permissions), and an api that exposes various operations on
his memory such as allocations, frees, reads and writes. The permissions region is necessary
because this emulator makes use of byte-level permission checks. These are done by maintaining
an entire memory space used purely for permissions.

The mmu exposes a custom alloc/free implementation that in combination with the byte level memory
permissions detects most relevant heap bugs (double free, uaf, heap buffer overflow).

Most aspects of the mmu are not yet relevant such as dirty bit resets for massive performance
increases or snapshot based memory resets, so I will hold off on implementing these until sfuzz is
ready to start fuzzing.


#### Code Generation

This emulator makes use of a custom just in time compiler for all of its execution. The code
generation is a multi-step process that will hopefully lead to a 20-50x performance increase over
pure emulation once simple optimizations are applied.

Once execution is started, each individual emulator thread has the ability to compile new code.
Whenever the emulator runs into a function that we have not yet compiled it invokes a mutex on the
JIT code backend and attempts to compile the entire function into the JIT backend before resuming
execution. While this mutex does have performance costs due to stopping all other threads,
it allows
us to have one memory region of code that all threads can make use off, thus only having to compile
each function once which ultimately leads to both memory and performance increases. Once the
compilation is completed, the muted is unlocked and the address of the newly generated code is
returned to the calling emulator which can then resume execution.<br><br>

`1. Lift the function to the intermediate representation` (Completed)

The first step of actual code generation is to lift the entire function into an intermediate
representation. The size of the function is determined during the initial target setupin main by
parsing the elf header and setting up a hashmap mapping function start addresses to their
sizes. This process basically just iterates through all original instructions and creates an IR
instruction based on the original instruction using large switch statement. The below example
imitates how the intermediate representation may look like for a very minimal function that
pretty much just performs a branch based on a comparison in the first block.
```
Label @ 0x1000
0x001000  A0(0) = 0x14
0x001004  A1(0) = 0xA
0x001008  if A0(0) == A1(0) (0x100C, 0x1028)

Label @ 0x100C
0x00100C  A2(0) = A0(0) + A1(0)
0x001010  A3(0) = 0x1
0x001014  Jmp 0x1018

Label @ 0x1018
0x001018  Z1(0) = 0x5
0x000000  A4(0) = A2(0) + Z1(0)
0x00101C  Z1(0) = 0x1
0x000000  A5(0) = A4(0) + Z1(0)
0x001020  Z1(0) = 0x0
0x000000  A6(0) = A3(0) + Z1(0)
0x001024  Jmp 0x1034

Label @ 0x1028
0x001028  A2(0) = A0(0) - A1(0)
0x00102C  A3(0) = 0x2
0x001030  Jmp 0x1018

Label @ 0x1034
0x001034  Ret
```
<p style="text-align:center;"<i>F1</i></p>

There are a couple of things to note here that may not be obvious. The addresses in the left
column represent the addresses from the original RISCV executable. Since this is a JIT compiler,
these addresses need to be maintained to dynamically translate jumps to functions that we
may not yet have compiled. Some of the addresses above however list 0x0. This is because the
intermediate representation sometimes needs more instructions to represent an original RISCV
instruction and thus only the first IR instruction corresponding to a RISCV instruction gets
an address. Also note how the register names still represent the original RISCV registers
(apart from Z1/Z2 which are temporary registers used by the IR). This is important to correctly map
special registers such as the Zero register or distinguish between callee-saved/caller-saves
registers later. Finally all registers have a '(0)' appended to them. This does not yet serve a
purpose, but will be necessary to hold additional information during the next step.
<br><br>

`2. Generate single-static-assignment form for the IR` (Completed)

The next step is to lift the previously generated code into single static assignment form. In this
form each variable is assigned exactly once. This is where the second field of each register comes
in. It is basically a counter for each register used to "create" a new register each time the
register is redefined. This creates some problems if a join point after a branch needs to make use
of a register that differs depending on which branch was taken (eg. in branch 1, `A1(1) = 5` is
executed while in branch 2 `A1(2) = 10` is executed). In this case the succeeding block does not
know on which version of A1 to operate on. Continuing with the above example, the phi function at
the beginning of the join block would look like this: `A1(3) = Phi(A1(1), A1(2))`. The computer
obviously does not have such as instruction, or ssa-form register usage so it needs to eventually
be deconstructed, nevertheless, this ssa representation is very frequently used in compilers
because it provides many advantages when attempting to run optimization passes on the code.

The below graph showcases how this form would look like for the above program. Note how the second
field of each register is now filled to make sure each register is only defined once, and that the
final block in the function now has phi-functions at its beginning for each register that it may be
required for.<br><br>

<p style="text-align:center;"><img src="resources/graph.png" alt="Dominator Tree" height="75%"
width="75%"/></p>
<p style="text-align:center;"<i>F2</i></p>

In this project ssa form is generated using the techniques proposed in
[Efficiently Computing Static Single Assignment Form and the Control Dependence
Graph](https://www.cs.utexas.edu/~pingali/CS380C/2010/papers/ssaCytron.pdf) by Cytron et al.

This algorithm makes use of dominance frontiers to compute a semipruned ssa representation that has
fewer phi-functions than more naive implementations that may just place phi-functions in
succeeding blocks for every register that survives block boundaries.

In my implementation, the steps to generate this ssa form are divided up into 4 main phases.

* Generate dominator tree

    In this phase, given a block b in the control flow graph, the set of blocks that strictly
    dominate b are given by (Dom(b)-b) where Dom(b) determines all blocks that must be traversed
    starting at the root of the cfg to get to block b. In this set the block that is closest
    to b is b's immediate dominator which is what we care to extract in this phase. This means
    that each cfg block exists in this form and that if a is the immediate dominator of b,
    an edge exists from a to b.

    The corresponding dominator tree for the above program is shown below. The first block
    dominates
    the 2 branching blocks as expected, but unlike in the cfg representation, here an edge exists
    from the first block to the join block because it is the earliest block that strictly dominates
    it.

<p style="text-align:center;"><img src="resources/domtree.png" alt="Dominator Tree" height="75%"
width="75%"/></p>
<p style="text-align:center;"<i>F3</i></p>

* Find the dominance frontier

    The dominance frontier is used to determine which registers require phi-functions for a given
    block. It starts by identifying all join points j in the graph since these are the only blocks
    that may potentially require phi-functions. Next it loops through all of the cfg-predecessors
    of each block j until iDom(j) is found. During this traversal, block j is added to the
    dominance frontier set of each block encountered in this process with the exception of iDom(j).

    This leads to the following dominance frontier for the above program which tells us that
    block 1
    and 2 may need phi functions to be placed in block 2 (block 1 & 2 represent the 2 branches from
    the original CFG as indicated by the labels).
    ```
    Label @ 0x1000 : {}
    Label @ 0x100c : {2}
    Label @ 0x1018 : {}
    Label @ 0x1028 : {2}
    Label @ 0x1034 : {}
    ```
<p style="text-align:center;"<i>F4</i></p>

* Insert phi functions into the graph

    Now that we know where we want to place phi functions, they need to actually be placed for
    registers that require them. Since we have the dominance frontiers we can determine this fairly
    well without accidentally placing many unnecessary phi-functions. For every definition x in
     block b, a phi-function needs to be inserted at every node in the dominance frontier of
    b. Since
    the insertion of a phi-function alters the instruction state, it may force the insertion of
    additional phi-functions. This process needs to restart after every phi-function insertion.

    This results in 2 phi-functions being insterted at the start of block 2 as showcased in the
    F2.

* Rename all registers to their appropriate ssa form

    In this phase the ssa form is completed by finally renaming all registers to their ssa-form
    name. Each register R with multiple definitions will thus be renamed R(1), R(2), ... R(n). This
    is done by maintaining a count of the highest-count definition of a register that is
    incremented whenever a new version of the register is defined alongside a stack that has
    the most recently defined version of the register on top of it.

    The algorithm used here walks through the dominator tree and for each block it starts by
    renaming all defined phi-functions definitions. Next it walks through each block in the
    program and rewrites the operands and declarations using the currently active ssa name for
    each register. For declarations, a newly generated ssa name must be created by incrementing its
    count variable and pushing it onto the registers stack. Finally the parameters of the phi
    functions of blocks succeeding the current block are renamed.

    Next it starts recursively calling the rename procecure on all children of the current
    block in the dominator tree. After this recursive call completes, all newly defined ssa
    registers are popped from each registers stack, thus resetting the register states back to
    the state prior to this blocks renaming procedure.

In the current state of the compiler, ssa representation does not yet serve much of a purpose
(although it can lead to better register allocation) since no optimizations have been written. This
form does however allow for powerful optimizations to be added in the future.<br><br>

`3. Potential Optimizations` (Not yet started)

Modern compiler backends employ many different optimizations to produce the best code possible. In
this case, due to limited time I will stick to very simple optimizations that are fairly
straightforward to implement while providing decent performance benefits such as eliminating all
instructions that attempt to write to the Zero register (basically a nop), or some basic constant
propagation to eliminate all temporary instructions that my IR added.<br><br>

`4. Register Allocation` (In progress)

The goal of this phase is to replace the previously set ssa instructions with standard x86\_64
registers. The main difficulty of this process is to correctly determine efficient register
allocation strategies that result in the least amount of registers being spilled to memory. This
phase is still very early in development, and I am not entirely sure how I want to implement
it yet.

* Computing liveness sets
This is the first part of register allocation, and is pretty much finished at this point. The goal
here is to determine which sets of registers are alive at the beginning of a block, and which
registers will remain alive coming out of a block. "Alive" in this context referring to a register
still being in use. To accomplish this I implemented Algorithms 4 & 5 from [Computing Liveness
Sets for SSA-Form Programs](https://hal.inria.fr/inria-00558509v2/document) by Brandner et al. This
algorithm starts at the uses of each register, and starts traversing the blocks backwards, filling
in each block's live-in and live-out sets as appropriate until the registers declaration is found.
Below you can once again see the results of this phase applied on the previously generated SSA-Code
```
Block 0:
live_in:  {}
live_out: {Reg(A0, 1), Reg(A1, 1)}

Block 1:
live_in:  {Reg(A0, 1), Reg(A1, 1)}
live_out: {Reg(A2, 1), Reg(A3, 1)}

Block 2:
live_in:  {Reg(A2, 2), Reg(A3, 2)}
live_out: {}

Block 3:
live_in:  {Reg(A0, 1), Reg(A1, 1)}
live_out: {Reg(A2, 3), Reg(A3, 3)}
```
<p style="text-align:center;"<i>F5</i></p>

* For the next part I am thinking of using parts from: [Linear Scan Register Allocation on SSA
Form](http://citeseerx.ist.psu.edu/viewdoc/download;jsessionid=018086FDA4BF35452D6324C96C3EC9D1?doi=10.1.1.162.2590&rep=rep1&type=pdf),
to implement a linear scan register allocator, however, I am still uncertain.


<br><br>
`5. Compiling to x86 machine code` (Not yet started)
<br><br>

#### References
* Emulation based fuzzing - Brandon Falk
* Cranelift [https://cfallin.org/blog/] - Chris Fallin
* Rv8: a high performance RISC-V to x86 binary translator - Michael Clark & Bruce Hoult
* Generating Low-Overhead Dynamic Binary Translators - Mathias Payer & Thomas R. Gross
* Efficiently Computing Static Single Assignment Form and the Control Dependence Graph - Cytron
et al
* Engineerining a compiler Keith D. Cooper & Londa Torczon
* Computing Liveness Sets for SSA-Form Programs - Brandner et al
* Linear Scan Register Allocation on SSA Form - Christian Wimmer & Michael Franz
* http://web.cs.ucla.edu/~palsberg/course/cs132/linearscan.pdf
* Basic-Block Graphs: Living Dinosaurs? - Jens Knoop et al
* RISCV User ISA specification
