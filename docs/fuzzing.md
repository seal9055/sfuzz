# Fuzzing Capabilities

#### Overview
This will probably be the most interesting aspect for most people looking to use this fuzzer. Here
I will describe the details of which features this fuzzer currently supports and their basic 
implementation details. 

#### Byte Level Permission Checks
This is covered in [memory_management.md](https://github.com/seal9055/sfuzz/tree/main/docs/memory_management.md), 
so I will not repeat the information here.

#### Coverage Tracking

This fuzzer implements both edge, block and call-stack based coverage tracking. The latter is not currently
used in any way, but available if usage is desired. Coverage is currently being tracked in a very 
simple way. A bytemap is maintained to determine which edges/blocks have already been hit. At the beginning of
each block, a fast hash is generated to index into the bytemap and check if the block/edge has already previously
been hit. If it has, we just move on. If it is a new edge/block however, the byte is set in the map, and the 
coverage counter is incremented to showcase that new coverage has been hit. For edge coverage this hash consists
of a quick xorshift hash, and for block-level coverage, the lower 24 bits of the address are just used. These
techniques are far from optimal, but they are easy to implement and work moderately well.

#### Coverage Guided Fuzzing

This is done in pretty much the simplest way possible. Whenever a case registers new coverage, it is added
to the corpus.

#### Persistent-mode/Snapshot Fuzzing

This is mostly a performance optimization, but since it is very specific to fuzzing I figured this
category probably suits it best. The basic idea behind persistent fuzzing is that the standard
`fork() + execve()` routine used for fuzzing is slow. 

One initial improvement afl uses is the forkserver optimization, where new processes are cloned
from a copy-on-write master that is kept in the original state. This reduces a lot of the overhead,
but still requires the expensive fork() syscall. A better alternative is to instrument the api with
a custom-written, single-process loop, therefore removing all of the 'execve()/fork()' overhead. AFL
mostly automates this, but still requires the user to write a small harness to designate where this
loop should be positioned.

In the case of sfuzz, since the fuzzer is running in an emulator, this becomes almost trivial. We
can specify a specific address as the snapshot starting-point, run the JIT up to that point, and
take a snapshot of the entire register/memory state. All future fuzz-cases can now use this snapshot
as their starting location instead of having to restart the process from the very beginning. This
can be used to avoid a lot of setup that is disconnected from our fuzzing input and thus greatly
speed up the fuzzing process. This becomes especially useful when dealing with larger targets, for
which we can take a snapshot right before the interesting function, set an exit-point right
afterwards, and then fuzz this function in a very tight/fast loop.

In the small test2.c target, placing a snapshot right after the open() call to skip the initial
setup/file opening, already led to a 30-50% speedup depending on the number of active threads.

#### Seed Scheduling

Seed scheduling is implemented based on power schedules. The inputs exist in a queue that is iterated
through. Before an input is executed, its energy is calculated. This determines how often an input will 
be executed (20000 to 150000 times based on its energy). The energy is kept within a reasonable range
to make sure no cases are completely left out, and that a case executes often enough that the cost of
this seed scheduling does not matter. This simply gives slight priority to favored cases.

The energy of a case is determined based on the input size (in bytes), execution time (measures in instructions
executed), and how frequently the case has found new coverage. Small sizes/execution times are favored, with new
coverage providing additional bonus points.

For the most part I don't think this strategy matters too much, so I deciced to only slightly favor "better" cases
over others, since especially at the start of a fuzzing campaign with an unfamiliar target, it is very hard to generalize
which metrics are actually important. Slower inputs could end up finding many more new code paths than faster inputs.

#### Mutation Strategies

The fuzzer currently has 8 different mutation strategies that are listed and described below.

- ByteReplace - This strategy replaces 1-128 bytes in the input with random other bytes. Smaller corruptions are 
  heavily favored over larger corruptions to avoid potentially destroying a good initial corpus.
- Bitflip - This strategy flips 1-128 random bits in the target. Smaller corruptions are once again heavily favored.
- MagicNum - This strategy maintains a small dictionary of hardcoded useful values. These are 1-8 byte values that lie 	  on the boundaries of integer over/underflows, and can thus frequently find integer bugs.
- SimpleArithmetic - This strategy simply adds or subtracts a random value from 1-32 to 0-128 random bytes in 
  the fuzzcase. This technique has proven to be very useful in the past and can often find integer bugs or corrupt 
  length fields.
- RemoveBlock - This strategy removes a random block from the input. It is more expensive than many 
  of the other strategies.
- DupBlock - This strategy duplicates a random block from the input. It is more expensive than many 
  of the other strategies.
- Resize - This strategy resizes the input. Decreasing the size simply truncates the input, while increasing the size
  adds random bytes to the end.
- Havoc - This strategy is invoked every 100 cases and simply combines multiple of the above listed strategies 
  together for a single case.

These mutation strategies are weighted. By default the cheaper/less destructive mutation strategies are 
favored (ByteReplace, Bitflip, MagicNum, SimpleAirhmetic), while the more expensive/more destructive
strategies are prioritized a lot less (RemoveBlock, DupBlock, Resize).

#### Crashes

Crashes are saved using a couple different methods to differentiate between different crashes. The different
crash causes are ReadFaults, WriteFaults, ExecFaults, OutOfBounds accesses and Timeouts. Timeouts occur when
a fuzz case executes more instructions that the timeout allwos. This is automatically calibrated using the initial
seeds, but can also be manually overridden in the configs. 

Unique crashes are based on the type of crash and the address that the crash occured on, and only unique crashes are saved off.