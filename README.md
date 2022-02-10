# sfuzz

Emulator in development

Requires nightly rust compiler to install

#### Overview

This project focuses on high performance RISC-V to x86\_64 binary translations for the purpose of
fuzzing.

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
returned to the calling emulator which can then resume execution.

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
<i>F1</i>

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

`2. Generate single-static-assignment form for the IR`(Completed)

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
required for.

![SSA-Form](./resources/graph.png =600x)

<i>F2</i>

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

    ![Dominator Tree](./resources/domtree.png =500x)

    <i>F3</i>

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
    <i>F4</i>

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
form does however allow for powerful optimizations to be added in the future.

`3. Potential Optimizations` (Not yet started)

Modern compiler backends employ many different optimizations to produce the best code possible. In
this case, due to limited time I will stick to very simple optimizations that are fairly
straightforward to implement while providing decent performance benefits such as eliminating all
instructions that attempt to write to the Zero register (basically a nop), or some basic constant
propagation to eliminate all temporary instructions that my IR added.

`4. Register Allocation` (In progress)

`5. Compiling to x86 machine code` (Not yet started)
