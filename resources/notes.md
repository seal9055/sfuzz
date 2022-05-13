#### These are just some random notes I'm taking while thinking about how I want to develop certain parts.


## Mutations

> https://lcamtuf.blogspot.com/2014/08/binary-fuzzing-strategies-what-works.html  
> https://www.usenix.org/system/files/sec19-lyu.pdf  
> https://www.usenix.org/system/files/raid2019-wang-jinghan.pdf [21 , 28, 29 , 37 , 54]

1. bitflips (1, 2, 4)
2. Byte flips (1, 2, 4)
3. add/sub integers (+-35), byte, word, dword, qword (signed & unsigned)
4. Insert common breaking points (-1, MAXINT, etc)
5. Increase/decrease size
6. Splice test cases together
x. Havoc

Setup dictionary  
> https://lcamtuf.blogspot.com/2015/01/afl-fuzz-making-up-grammar-with.html  

## Seed Scheduling

Analysis:
1. Graph centrality
2. Mutation history to determine when to stop focusing on "hard" edges

Dynamic:
1. Execution time
2. Less bytes
3. Coverage
4. How often the seed has been chosen
5. Number of inputs with same cov
6. Generated test cases based on this input with same cov

-
1. Decrease whenever no new cov is found

## Coverage eval

> [Cov-sensitivity] https://www.usenix.org/system/files/raid2019-wang-jinghan.pdf
    - Tracking call stack seems kinda sick. n=2-gram cov seems nice too
    - Assign different coverage metrics to different threads and synch corpus
    - Assign different coverage metrics to different threads and dont synch

> [Cerberos] https://dl.acm.org/doi/pdf/10.1145/3338906.3338975
    - Complexity score calculated for each function that can be correlated to inputs via their 
    coverage trace

    - Idea is to assign rank using the 5 metrics below, and to then queue up all seeds based on 
    their weight. Uses paretto frontier

    - exec time, number of covs, unique edges, file size, complexity score

> [Directed fuzzing] https://dl.acm.org/doi/pdf/10.1145/3133956.3134020
    - Analyze program callgraph/cfg to direct the fuzzer to specific target points in the program

> https://www.ndss-symposium.org/wp-content/uploads/2020/02/24422-paper.pdf
    - Don't treat all coverage equally, label security-relevant edges based and assign weights based
    on their path to vulnerable functions (eg. memcpy or a lot of memory operations)

> https://arxiv.org/pdf/2203.12064.pdf
    - Graph centrality analysis

> https://mboehme.github.io/paper/CCS16.pdf
    - AFLFast
    - More energy to low frequency paths
    - Model seed scheduling as markov chains

> https://www.usenix.org/system/files/woot20-paper-fioraldi.pdf
> https://dl.acm.org/doi/pdf/10.1145/3133956.3134073
> https://www.ndss-symposium.org/wp-content/uploads/2017/09/ndss2017_10-2_Rawat_paper.pdf

Things to potentiall maintain for each input to determine energy:
    - Size
    - Execution time
    - Which cov-units it hits & how rare each of them is (potentially complexity score for each
        cov-unit as well)
    - 
    
Cull corpus:
    - Track which cov points an input hits to potentially dedup/remove obsolete cases

    - Periodically cull entries that are superseeded by other entries (maybe check how often the 
        entry was hit too)

Add case to inputs:
    If a case produces same cov, but block is executed different number of times as
        previous cases, it is regarded as interesting

Timeout:
    - Timeout: 5x initial calibrated exec speed

Scheduling:
    - Whenever a seed is chosen, execute it n times instead of just once
    - Alternatively just go through all seeds sequentially and execute each seed n times

Calculate score:
    Initial:
        - Base it on size & exec-time

    Dynamic:
        1. 
            - weight = number of cov points an input hits
            - weight += 25% / unique/new cov
            - weight += bonus (based on shorter exec/shorter input)
            - avg_weight = average of all input weights
            - Assign each input weight based on how far above/below the avg they are
        2.
            ```rs
            fn calculate_energy(input: Input) {

            }

            fn fuzz_loop() {
                loop {
                    id = get_next_seed();
                    p = calculate_energy(corpus.inputs[id]);

                    for i in 0..p {
                        mutate(corpus.inputs[i]);
                        fuzz(corpus.input[i]);
                    }
                }
            }
            ```


## Crash deduping
1. AFL
    - Crash trace includes a tuple not seen before
    - Crash trace is missing a tuple seen before

2. stuff
    - Dont threadshare the crash_map, and instead work by sending crashing inputs to the main 
    thread to handle


## Other
> https://arxiv.org/abs/2009.06124 - Scaling  

> Coverage-tracking
    - priority queue, increase priority of input whenever it finds new coverage
```rs
    let hash = calculate_hash(from, to);
    hash &= bitmap_size;
    let idx = hash / 64;
    let bit = 1 << (hash % 64);
    if (state->cov_bitmap[idx] & bit) == 0 {
        state->cov_bitmappidx] |= bit;
        state->exit_reason = 7;
        state->cov_from = from;
        state->cov_to   = to;
        state->reenter_pc = pc+4;
        return
    }
```
