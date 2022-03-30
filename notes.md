## TODO in codebase
##### Required
    1. Concurrency bug since I insert stuff into map during compilation

##### Optional Optimizations
    2. Hardcode most used registers for performance gains  
    3. Self-Modifying code  

## Fuzzing Reading List
    1.  y https://www.fuzzingbook.org/
    2.  y https://lcamtuf.blogspot.com/
    3.  n https://www.amazon.com/Fuzzing-Brute-Force-Vulnerability-Discovery/dp/0321446119
    4.  y https://lcamtuf.coredump.cx/afl/technical_details.txt

    5.  https://arxiv.org/pdf/2005.07797.pdf
    6.  https://wcventure.github.io/FuzzingPaper/Paper/TRel18_Fuzzing.pdf
    7.  https://www.amazon.com/Fuzzing-Software-Security-Assurance-Information/dp/1596932147
    8.  https://arxiv.org/pdf/2202.03643.pdf
    9.  https://www.ndss-symposium.org/wp-content/uploads/2020/02/24422-paper.pdf
    10. fitm: binary only coverage-guided fuzzing
    11. https://shadowmydx.github.io/papers/icse22-main-1314.pdf
    12. https://arxiv.org/pdf/2203.12064.pdf
    13. https://dl.acm.org/doi/pdf/10.1145/3460319.3464795
    14. https://www.usenix.org/system/files/raid2019-wang-jinghan.pdf
    15. collision free coverage vs https://alifahmed.github.io/res/BigMap_DSN.pdf
    16. https://link.springer.com/article/10.1007/s10664-020-09927-3
    17. https://chungkim.io/doc/ndss20-hfl.pdf

## Coverage guided fuzzing
##### Power Schedules
    > Assign different weights to inputs in the corpus to "smartly" distribute fuzzing time
    > Black box
        - AFL assigns more weight to inputs that are shorter, execute faster and result in more 
            frequent coverage increases
    > Gray Box

##### Basic block coverage
    > Track coverage whenever a new basic block is hit

##### Edge coverage 
    (<id of cur code location>, <id of previous code location>)
    > Generate tuples of the above form for each piece of code. If a new tuple is encountered, add
        the mutated input as a new corpus entry (A -> B, simplest hash would be A ^ B)
    > Generally better than block coverage since it provides more insight into program execution
        - Can trivially distinguish between the following 2 paths
          A -> B -> C -> D -> E (tuples: AB, BC, CD, DE)
          A -> B -> D -> C -> E (tuples: AB, BD, DC, CE)
    > Could count how often each edge is taken?

    cur_location = <COMPILE_TIME_RANDOM>;
    shared_mem[cur_location ^ prev_location]++; 
    prev_location = cur_location >> 1;

##### Trace/path Coverage
    > Number of logical paths in the program that were taken during execution
    > Measures progress by computing a hash over the branches exercised by the input
    > Can be used to estimate how much more coverage will be gained/time with further fuzzing
    > Issues
        - Potentially infinite paths given loops

##### Collision Free coverage strategies


## Mutational Strategies
##### General approach
    > Feedback loop approach
        - Measure what type of mutations result in new coverage and use them more frequently
    > Start with sequential deterministic mutations before moving on to randomness

##### Individual strategies
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

## Triaging Crashes
##### Crash exploration
    > Used to more easily understand what exactly caused a crash
    > Entirely separate mode that takes a crashing input and looks for more inputs that cause the
        same crash by mutating this input. This process uses very similar methods as the main
        fuzzer. Eventually it will have generated a small corpus of inputs related to the bug that
        can be triaged together to better understand the bug

##### Deduping Crashes
    > Group "similar" crashes together
        - With edge based coverage this can be done whenever a new tuple is found that hasnt been
            used to achieve this crash before, or if a tuple is missing
    

## Performance
##### Persistent mode / Snapshot fuzzing
    > Implement mechanisms similar to copy-on-write/dirty bit memory resets to avoid having to reset
        large amounts of memory. This allows for much faster fuzzing.

## Symbolic Fuzzing
    > Extract values used in cmp instructions affecting control-flow and add these to a known
        dictionary that fuzz cases can use to achieve new control flow
    > To keep up with the performance a fuzzer requires to be effective, this symbolic execution is
        often only done at a very shallow scale
    > Mostly useful when dealing with a lot of magic values that need to be bypassed to achieve more
        coverage

