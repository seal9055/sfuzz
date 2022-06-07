# Code Generation


**Small Note**
```
This was by far the most time-consuming and difficult aspect of this entire project. I initially 
spent about 3 months trying to format this like a proper optimizing compiler might. This included
lifting the code to an intermediate representation, transforming it to single-static-assignment form,
performing register allocation and finally compiling it to x86_64 machine code. I implemented all of
these, but in the end I decided to fall back to a simpler approach due to multiple reasons I outline
below. I still believe that this approach is possible though and holds decent performance gains, so 
I will most likely reattempt this in the future.
```

#### Overview

This emulator makes use of a custom just-in-time compiler for all of its execution. The code generation is a multi-step process that leads to a 20-50x performance increase over pure emulation. 

Once execution is started, each individual emulator thread has the ability to compile new code. Whenever the emulator runs into a function that we have not yet compiled it invokes a lock on the JIT code backend and attempts to compile the entire function into the JIT backend before resuming execution. This lock only stops other threads from adding new code to the JIT-backing during compilation without stopping them from using the JIT-backing. This means that one thread compiling new code has basically no impact on any of the other threads, making this lock mostly free while providing 1 uniform memory region that contains all of the compiled code for all threads. Once the compilation is completed, the mutex is unlocked and the addresses of the newly generated code are added to the JIT lookup table. At this point, the compiling thread can resume fuzzer execution and all other threads can access this newly compiled code via the translation table.

Most of the code pertaining to code-generation can be found in [jit.rs](https://github.com/seal9055/sfuzz/blob/main/src/jit.rs), [irgraph.rs](https://github.com/seal9055/sfuzz/blob/main/src/irgraph.rs), and [emulator.rs](https://github.com/seal9055/sfuzz/blob/main/src/emulator.rs). More detailed descriptions of some of these processes are provided below.

#### Lifting a Function to Custom IR
The first step of actual code generation is to lift the entire function into an intermediate representation. The size of the function is determined during the initialization phase when first loading the target. This is done by parsing the elf metadata and setting up a hashmap mapping function start addresses to their sizes.<br>

The IR-lifting just iterates through the original instructions and creates an IR instruction based on the original instruction using a large switch statement. The below example imitates how the intermediate representation may look like for a very minimal function that pretty much just performs a branch based on a comparison in the first block.
```
Label @ 0x1000
Label @ 0x1000
0x001000 A0 = 0x14
0x001004 A1 = 0xA
0x001008 if A0 == A1 (0x100C, 0x1028)

Label @ 0x100C
0x00100C A2 = A0 + A1
0x001010 A3 = 0x1
0x001014 Jmp 0x1018

Label @ 0x1018
0x001024 Jmp 0x1034

Label @ 0x1028
0x001028 A2 = A0 - A
0x00102C A3 = 0x2
0x001030 Jmp 0x1018

Label @ 0x1034
0x001034 Ret
```
<p style="text-align:center;"<i>F1</i></p>

At this point, I attempted a couple of different approaches before settling on the current code generation procedure. My first approach was to first transform the above IR code into single static assignment form. This allows for stronger optimizations and is a very popular choice for modern compilers. Next, I used a linear scan register allocator to assign registers to the code and compile the final code.

This approach resulted in multiple issues that led to me eventually abandoning it in favor of the current implementation. Some of the reasons as to why I changed my approach are listed below.

1. **Debugging** - Since this is meant to be a fuzzer, being able to properly debug crashes, or at least 
    print out register states is important. After doing register allocation, determining which x86 register is allocated to each RISCV register at runtime to print out useful information is very difficult.

2. **Extendability** - When it comes to register allocation, a lot of the backend features (eg. A0-A7 for 
    arguments, or syscall number in A7) are architecture-dependent. This makes it a lot harder to write the backend in a way that can be extended with new architectures by just adding a front end.

3. **Performance** - In theory, the ssa/regalloc approach will lead to better final code. In this case, 
    however, since it's a binary translator, a lot of registers such as function arguments or stack pointers have to be hardcoded to x86 registers since we don't have important information such as the number of arguments when translating binary -> binary. This in addition to the meta-data required by the JIT (pointer to memory, permissions, JIT lookup table, register spill-stack, etc) led to most x86 registers being in use, leaving only 4 x86 registers available for the actual register allocation in my approach. This could obviously be greatly improved upon, but this would require a lot more time to achieve comparable results.

4. **Complexity** - This approach added a lot of extra complexity to the project which caused major 
    issues and would have delayed the completion of this project by several months to debug all of these issues

Nevertheless, I did implement both ssa-generation and register allocation before eventually abandoning it, and since it was a very large part of my time investment I decided to still keep notes on it. The implementation details are listed in the below 'Optimizing Compiler' section, and the final code for this approach can be viewed at commit 7d129ab847d171b66901f4c936dd2ad5c5a1b79a on the Github repository.

#### Compiling to x86 Machine Code

This phase pretty much just loops through all the previously lifted IR instructions and compiles them to x86 code. Whenever a syscall or a hooked function is encountered, appropriate instructions are generated to leave the JIT and handle the procedure. All registers are currently memory-mapped within the emulator. While this would have a very significant performance impact for normal programs, in the case of a fuzzer I can use the free'd registers up through this approach to point to other important frequently accessed fields such as dirty lists or instruction counters, so in the end, the performance overhead incurred by this is negligible.

In addition to the previously mentioned actual code compilation, a lot of other very important steps are taken at this point. Mainly, the RISC-V to x86 translation table is populated, and instructions to instrument the code for fuzzing are inserted to enable snapshotting, coverage, hooks and proper permission checks. 
<br>  

## Optimizing Compiler

#### Generate SSA-form for the IR

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

<p style="text-align:center;"><img src="../resources/domtree.png" alt="Dominator Tree" height="75%"
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

#### Potential Optimizations

Modern compiler backends employ many different optimizations to produce the best code possible. In
this case, due to limited time I will stick to very simple optimizations that are fairly
straightforward to implement while providing decent performance benefits such as eliminating all
instructions that attempt to write to the Zero register (basically a nop), or some basic constant
propagation to eliminate all temporary instructions that my IR added.<br><br>

#### Register Allocation

The goal of this phase is to replace the previously set ssa instruction operands with standard
X86\_64 registers. The main difficulty of this process is to correctly determine efficient register
allocation strategies that result in the least amount of registers being spilled to memory. This
phase is still very early in development, and I am not entirely sure how I want to implement
it yet.

* Instruction Numbering

The first step is to number the instructions. This assigns a unique id to each instruction. The main
thing to consider here is that instructions need to be ordered in order of execution. This means
that every instruction A that is executed before instruction B needs to have a lower id. This can be
accomplished using the previously generated dominator tree's.

* Register Live Intervals

The goal of this phase is to determine how long each register is alive. For each used register it
computed an interval from the point that the register is first defined to its last usage according
to the previously marked id numbers during the instruction numbering phase.

* Linear Scan Register Allocation

This algorithm is pretty much the simplest way to do register allocation across block boundaries.
Nevertheless it is the most popularly used register allocation algorithm in JIT compilers since it
results in low compile time which is an important metric for JIT compilers. Additionally it only
produces slightly worse code than much slower algorithms such as graph coloring approaches.

The pseudo-code for this register allocation approach is listed below. We loop through all
previously determined register liveness intervals and allocate an X86 register as long as there are
free registers are available. If there is no free register available, the last used register is
spilled to memory to obtain a free register.

```rs
for (reg, interval) in live_intervals { // in order of increasing starting point
    // Start by expiring old intervals by removing all no longer in use registers from the active
    // mapping and adding it to the free registers instead.
    expire_old_intervals();

    if free_regs.is_empty() {
    // Need to spill register to memory if there are no more free registers available
        // Spill the register with the farthest use
        spill_reg = active.pop();

        // Use the now free'd register for the current register
        mapping.insert(reg, spill_reg);

        // Insert new range to active range
        active.insert(spill_reg, inter);
    } else {
    // Free register available, so just add it to the mapping
        preg = free_regs.pop();
        active.insert(preg, inter);
        mapping.insert(reg, preg);
    }
    return mapping;
}
```

#### Future Work
As mentioned previously I would like to re-explore the optimizing compiler approach in the future. I believe it has a lot more potential than the more naive implementation, but it is not an immediate priority because there are more important improvements that I want to tackle first.
