## Fuzzing Reading List
    1.  Basics          https://www.fuzzingbook.org/
    2.  Basics          https://wcventure.github.io/FuzzingPaper/Paper/TRel18_Fuzzing.pdf
    3.  Afl-dev         https://lcamtuf.blogspot.com/
    4.  Afl-dev         https://lcamtuf.coredump.cx/afl/technical_details.txt
    5.  Afl-study       https://www.s3.eurecom.fr/docs/fuzzing22_fioraldi_report.pdf
    6.  Cov-sensitivity https://www.usenix.org/system/files/raid2019-wang-jinghan.pdf
    7.  Valued coverage https://www.ndss-symposium.org/wp-content/uploads/2020/02/24422-paper.pdf
    8.  CFG-Seed sched  https://arxiv.org/pdf/2203.12064.pdf
    9.  Seed selection  https://dl.acm.org/doi/pdf/10.1145/3460319.3464795
    10. Havoc           https://shadowmydx.github.io/papers/icse22-main-1314.pdf
    11. Feedback-muts   https://link.springer.com/article/10.1007/s10664-020-09927-3
    12. Snapshots/state https://arxiv.org/pdf/2202.03643.pdf
    13. Snapshots/state https://github.com/fgsect/FitM/blob/main/fitm.pdf
    14. Baseband-emu    https://arxiv.org/pdf/2005.07797.pdf
    15. Benchmarking    https://github.com/google/fuzzbench/issues/654
    16. Benchmarking    https://hexgolems.com/2020/08/on-measuring-and-visualizing-fuzzer-performance/
    17. Crash-triaging  https://www.usenix.org/system/files/sec20-blazytko.pdf
    18. Redqueen        https://synthesis.to/papers/NDSS19-Redqueen.pdf
    19. Nautilus        https://wcventure.github.io/FuzzingPaper/Paper/NDSS19_Nautilus.pdf
    20. AFL++           https://www.usenix.org/system/files/woot20-paper-fioraldi.pdf
    21. Hash-collisions https://chao.100871.net/papers/oakland18.pdf
    22. Bigmap-covmap   https://alifahmed.github.io/res/BigMap_DSN.pdf

## Corpus Management

##### Corpus Minimization
    > Some fuzzers such as afl trim their corpus' to discard long inputs that take the same path as 
        shorter inputs.

    > Pros
        - Cut down duplicate entries to not waste time on cases that don't provide more information
        - Smaller inputs are executed faster leading to higher performance
    > Cons
        - You potentially discard corpus entries that contained a valuable input
        - Reducing the size of inputs can greatly reduce the "state" the input has going into a
            specific block, thus leading to less bugs even if the same edges are covered

    > Specific techniques
        - Minset: compute weight by execution-time / file-size
        - Afl-cmin: Uses coverage information using tracked edge frequency counts
        - OptMin: Generates potentially optimal solutions unlike previous 2 approximations

##### Seed Selection
    > High quality initial seeds are very important because the originals can carry a lot of
        semantics that the fuzzer now no longer has to randomly generate or know. Any part that
        isn't covered by the corpus requires additional work on the side of the fuzzer to get there.
        This has a significant impact on expanding code coverage since a larger corpus already 
        covers many more cases as its base

##### Seed Collection
    > Web crawler to collect input files
    > Seed-collections:
        - https://datacommons.anu.edu.au/DataCommons/rest/records/anudc:6106/data/
        - https://lcamtuf.coredump.cx/afl/demo/
        - https://github.com/radareorg/radare2-testbins

## Coverage Tracking 

##### Basic Block Coverage
    > Track coverage whenever a new basic block is hit

##### Edge Coverage 
    (<id of cur code location>, <id of previous code location>)
    > Generate tuples of the above form for each piece of code. If a new tuple is encountered, add
        the mutated input as a new corpus entry (A -> B, simplest hash would be A ^ B)
    > Generally better than block coverage since it provides more insight into program execution
        - Can trivially distinguish between the following 2 paths
          A -> B -> C -> D -> E (tuples: AB, BC, CD, DE)
          A -> B -> D -> C -> E (tuples: AB, BD, DC, CE)

    Example hash functions:
        - hash = (prev_block << 1) ^ cur_block
        - AFL Implementation:
            cur_location = <COMPILE_TIME_RANDOM>;
            shared_mem[cur_location ^ prev_location]++; 
            prev_location = cur_location >> 1;

##### N-gram Edge Coverage
    > Track latest n edges taken. Tracking only the current edge offers little information about the
        actually taken path, while tracking an infinite amount of edges could result in path
        explosion. Common values for n are 2, 4 or 8

##### Path Coverage
    > Number of logical paths in the program that were taken during execution
    > Measures progress by computing a hash over the branches exercised by the input
    > Can be used to estimate how much more coverage will be gained/time with further fuzzing
    > Can potentially lead to path explosion if eg. a large loop is found

##### Collision Free Coverage
    > Generally accomplished by assigning a unique value to each edge during instrumentation, so a
        coverage bitmap can be efficiently accessed using this hardcoded value instead of computing
        a hash that risks collisions

##### BigMap
    > Common strategy to lower hash collisions is to increase the table size, this however results
        in lower cache locality and can greatly reduce perf.
    > BigMap adds an additional level of indirection so randomly scattered coverage metrics are
        instead stored in a sequential bitmap to maintain the currently active region in caches

##### Data Coverage
    > Distinguish test cases from a data accessing perspective

##### Collection Methods
    > Code instrumentation to report coverage information
    > Intel PIN - jit compiles program as soon as it is loaded into memory while adding additional
        instructions to track coverage
    > Randomly request current location of fuzzer at certain time intervals to track which code is
        executed
    > Intel PT - Hardware branch tracer

## Seed Scheduling
##### Metrics:
    > Vulnerable paths 
        - Weight of each branch is based upon vulnerable functions (eg. memcpy) it can reach and the
            amount of loads/stores given different paths
    > Number of edges reachable from a given seed
    > Mutation history can be used to determine when one should stop focusing on "hard" edges
    > Graph centrality analysis - approximate number of reachable edges from given seed and give
        weight depending on how "central" a seed is.

##### Coverage Guided / Power Schedules
    > Assign different weights to inputs in the corpus to "smartly" distribute fuzzing time
        - Execution time
        - Shorter
        - More frequent coverage increases

## Mutational Strategies
##### General Approach
    > Feedback loop approach
        - Measure what type of mutations result in new coverage and use them more frequently
    > Start with sequential deterministic mutations before moving on to randomness
    > Target specific mutations will generally outperform generic mutation strategies. This can be
        enhanced by developing a state-concious fuzzer
    > Havoc: apply multiple randomly selected mutators simultaneously on some inputs

##### Individual Strategies
    > Walking bit flips - sequential ordered bitflips
        Pros:
            - Pretty good at finding low hanging fruit because it goes through entire input and gets
                a good bit of initial coverage
        Cons:
            - Expensive to keep up since each test requires 8 execve() calls per byte of the input
                file. Has diminishing returns, so only used for a short initial period.
    > Walking byte flips - sequential ordered byte-flips
        Pros/Cons: Much less expensive than bit flips, but also not very effective in the long run
    > Simple arithmetics - inc/dec integers in the input according to be/le and different sizes
        Pros:
            - Good to spot a lot of bugs pertaining to integer over/underflows or incorrect size
                checks
        Cons:
            - Relatively high costs (~20 execve calls per byte)
    > Known Integers - hardcoded set of integers that commonly trigger bugs (-1, 0, MAX_INT, etc)
        Pros/Cons: Very expensive, but can quickly find some common bugs before being disabled while
                     going through the small hardcoded list of known values
    > Stacked Tweaks - non deterministic random mutations
        - bit flips
        - random incs/decs for 8/16/32/64 byte values
        - random single byte sets
        - block deletion
        - block duplication
        Pros:
            Extremely simple to implement
            Surprisingly very effective at generating new coverage
    > Changing size of input
    > Dictionary: Maintain a dictionary (either statically defined or dynamically created during
        runtime) of interesting strings, that can be added to the input at random positions.
    > Splicing: Combine two different inputs at random positions

## Triaging Crashes
##### Crash Exploration
    > Used to more easily understand what exactly caused a crash
    > Entirely separate mode that takes a crashing input and looks for more inputs that cause the
        same crash by mutating this input. This process uses very similar methods as the main
        fuzzer. Eventually it will have generated a small corpus of inputs related to the bug that
        can be triaged together to better understand the bug
    > Once a lot of crashing inputs are gathered, statistical analysis can be performed on the
        crashing inputs to find common cases, and automatically extract a lot of possible crash
        reasons.

##### Deduping Crashes
    > Group "similar" crashes together to avoid looking at hundreds of similar crashes
        - With edge based coverage this can be done whenever a new tuple is found that hasnt been
            used to achieve this crash before, or if a tuple is missing

##### Debugging
    > The simplest, but also most manual labor intensive approach is to just load the crashing input
        into a debugger and to manually attempt to figure out the root cause.
    > This can be improved upon with modern timeless debuggers that provide reverse execution 
        functionality. This can be used to traverse the program backwards starting at the start
        location, which can often make bug triaging a lot more comfortable.

## Performance
##### Persistent Mode / Snapshot Fuzzing
    > Fuzz in a short loop around the specific target functions by saving the state right before the
        execution of the target, and then basing future fuzz cases off of this specific starting
        state instead of fully reseting the program on each run.
    > Can additionally implement mechanisms similar to copy-on-write/dirty bit memory resets to 
        avoid having to reset large amounts of memory. This allows for much faster fuzzing.

##### In-memory Fuzzing
    > Many fuzz-targets read input from disk before starting to operate on the
        data. This leads to poor scaling due to the heavy io usage. Instead a fuzzer can just load
        the corpus into memory and directly pass it into the binary to avoid the disk performance
        overhead.

##### Scaling
    > When used in the real world, fuzzers are generally ran on at least 50-100 cores. This means
        that not only does the fuzzer need good single-core performance, but it also has to scale
        well with a large number of cores.
    > If coverage information and corpus are meant to be shared between cores, they need to be 
        implemented in ways that can be shared between the threads without incurring high costs.
        This means that certain techniques that track massive amounts of additional information to 
        make improved decisions suddenly become unviable when attempting to scale because all of the
        information needs to be passed between cores.
    > Another common pitfall of scaling attempts is the kernel. If the main fuzzing loop contains 
        frequent syscalls, the kernel starts taking up a good chunk of the time that should be spent 
        fuzzing. This becomes increasingly relevant when running the fuzzer on a high number of
        threads, which can easily result in >40% of total execution time being wasted in random
        kernel locks.

## Symbolic Execution in Fuzzing

    > Heavy symbolic analysis is still too slow for fuzzers, however using approximations one can
    gain many of the benefits without the massive performance hit

##### CMP-Deconstruction
    > Extract values used in cmp instructions affecting control-flow and add these to a known
        dictionary that fuzz cases can use to achieve new control flow
    > Mostly useful when dealing with a lot of magic values that need to be bypassed to achieve 
        more coverage
    > Can be done via input-to-state correspondence between inputs and current program state. Start
        by finding all cmp instructions in an initial run and hooking all of them to retrieve the
        arguments. Using the compare operand, values that are likely to pass the check can be
        calculated (eg. zero/sign-extend, equal, slightly larger/less than, etc). The input is
        colorized to create lightweight approximation to taint tracking that can be used to track
        which part of the input finds itself in the cmp instruction.
    > Another approach is to transform multi-byte comparisons into multiple single byte comparisons,
        thus being able to leverage coverage guided fuzzing to bypass the check

##### Checksums
    > Checksums can be very challenging for fuzzers to handle since unlike magic-byte checks, they
        can change from one run to the other based on the provided input and greatly halt fuzzer
        progress.
    > One possible method is to statically identify checksum checks and patch them to a check that
        always returns true.

##### Concolic Execution 
    > Track all conditionals during execution, and collect conditional constraints. These can be 
        used to then produce inputs that take the non-traversed path. Still has a very real 
        performance impact, but it does not suffer from state explosion and can thus be implemented 
        in a scaling manner.

##### Taint-based Fuzzing
    > Tracks input-flow throughout a target to learn which parts of the input have an effect on
        certain operations. Can be used to eg. find magic bytes or integer overflow vulnerabilities,
        but has mostly been replaced in fuzzers by techniques that accomplish similar goals without
        the massive performance overhead that proper taint-tracking results in.

## Benchmarking Fuzzers
    > When profiling new algorithms in fuzzers, algorithmic performance (eg. coverage/cases) is much
        more relevant than timed performance (eg. coverage/time) due to the high variances that can
        occur using random fuzz-inputs. Time-performance is the most important aspect for finished
        fuzzers, but while benchmarking fuzzers in development it is unreasonable since it would
        require prototypes to be highly optimized to compete. This assumes that the developer can
        make reasonable assumptions about the performance implications of the algorithm once
        optimized.
    > Minor variables at the start of the fuzzer run can have massive impact on the rest. Eg. high
        corruption can lead to initially high coverage with strongly diminishing returns once the high
        corruption hits required bytes for further progress.
    > When properly evaluating fuzzers, debugging/introspection ability is extremely important
        rather than just running benchmarks/reviewing coverage graphs
    > Log Scale vs Linear Scale
        - Linear scale describes where a fuzzer flatlines, but doesn't produce much data otherwise
        - Much more coverage at the beginning of fuzzer-runs than at the end so a linear scale 
            results in a vertical increase at t=0 and an almost horizontal line for the rest of the 
            run which provides almost no information.
        - Log scales can make for easier interpretation of specific spikes during fuzzer runs
    > When benchmarking, don't focus on short fuzzer runs, but rather let the fuzzer run for eg. 24
        hours since some changes will have short term benefits but longterm drawbacks
    > Scaling is extremely important for real world fuzzer metrics. If a fuzzer performs better on
        single core metrics but then completely falls off when scaled to 50-100 cores it becomes
        unusable for proper fuzzing campaigns. Another point would be that it doesn't just scale
        across cores but also across multiple servers, eventhough this is potentially harder to
        test. A lot of proposed high introspection fuzzing techniques suddenly fall apart when faced
        with scaling because all of this data needs to be shared between cores.

##### Metrics
    > # of bugs is basically worthless because it relates more to the amount of hours spent using
        the fuzzer on bad targets instead of the actual fuzzer performance
    > Evaluating based on known bugs is useful if you are already familiar with the bugs and can
        thus determine if your fuzzer works as expected.
    > Coverage is probably the most popular metric to measure fuzzers. The proficiency of a fuzzer
        is often directly correlated with the amount of coverage it achieves. It might be misleading
        in certain cases such as grammar based fuzzers that only test a certain subset of an
        application.
    > Sampling based measurement to count how often individual blocks are hit by input. This
        provides information about how often blocks are reached, which is more valuable than
        single-hit coverage tracking.
    > State-aware coverage tracking: Measure which target states of a specific stateful target the
        fuzzer manages to hit.

## Grammar-based Fuzzing
    > Many applications that require highly structured inputs (eg. compilers) make fuzzing using
        mutational fuzzer implementations difficult. Grammar fuzzers in comparison generate input
        from scratch instead of modifying existing input. When fuzzing a javascript interpreter for
        example, a grammar based fuzzer would generate random but valid javascript code and use this
        as fuzz input. This greatly reduces the number of fuzz cases that would otherwise be
        immediately thrown out due to syntax errors with mutational engines.

## Misc

##### Crash Amplification
    > The goal of fuzzing is usually to find potentially exploitable bugs on a target.
        Unforunately fuzzers are generally only capable of finding these bugs if they actually cause
        a crash. The goal of crash amplification is to more easily crash the program if a bug
        occurs.

    > Compile-time instrumentation
        - ASAN: Address sanitization can be used to add runtime checks to the binary that track out
            of bounds accesses or heap bugs. Approximately 2x performance hit, but generally worth
            the extra cost.

    > Emulation
        - Byte level permission checks to catch off-by-one errors similar to asan
        - Hooking various functions such as malloc/free to instead replace them with safe
            alternatives that crash on any misuse

