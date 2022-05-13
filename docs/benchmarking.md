# Benchmarks & Testing

##### &nbsp;Since the fuzzer is not at a point where it can run proper benchmarks (eg. [fuzzbench](https://google.github.io/fuzzbench/)), this pretty much consists solely of sample programs I wrote and other programs I chose to include. Note that this means that my conclusions may be biased. If you believe that my comparisons were unfair for any of these test-cases, please reach out.

<br>

#### Single-threaded JIT Performance Against a Simple Target

**Experiment-Setup**  
This initial test just compares the fuzzer's performance when used on a very simple test binary. This test showcases the low overhead of the fuzzer when it comes to resetting memory and running many very short cases. Arguably this is not a very important test-case since no real program will be this simple, but I found it interesting nonetheless. The program basically just has the fuzzer jump through some small if-comparison's before segfaulting, thus giving the fuzzer a crash. Any coverage guided fuzzer should be able to trivially find the crash within seconds. The corpus consists of a 100-byte file generated from `/dev/urandom`.

For this test-case, all of my fuzzer's features are enabled, including coverage tracking, byte-level permissions, allocator-hooks, in-memory fuzzing, and snapshot based fuzzing. I set the snapshot right after the call to `open`.

I will be comparing my fuzzer's performance to AFL++. I will be testing both qemu-emulation and compile-time instrumentation for afl, and since this is a stateless target, I will also instrument the below code for persistent-mode
fuzzing and in-memory input generation to make it as fair as possible.
```c
int main(int argc, char **argv) {
    char buf[100];
    int fd = open(argv[1], O_RDONLY);

    read(fd, buf, 100);

    if (buf[0] == 0x41) {
      if (buf[1] == 0x42) {
        if (buf[2] == 0x43) {
          if (buf[3] == 0x44) {
            if (buf[4] == 0x45) {
              if (buf[5] == 0x46) {
                *(unsigned long*)0x4141414141414141 = 0;
              }
            }
          }
        }
      }
    }
    return 0;
}
```

**Results**

- <b>SFUZZ snapshot-based:</b> 1.8 million fuzz cases per second
- <b>SFUZZ no snapshot:</b> 750,000 fuzz cases per second
- <b>AFL++ QEMU default configs:</b> 3500 fuzz cases per second
- <b>AFL++ source instrumentation:</b> 3500 fuzz cases per second
- <b>AFL++ source instrumentation & persistent mode/in memory fuzzing:</b> 33,000 fuzz cases per second

SFUZZ finds the crash within the first second of running and executes about 1.8 million fuzz cases per second. Disabling snapshot based fuzzing and starting each test case at the `_start` function still finds the crash immediately, but performance drops to 750,000 per second. This massive gap is because this is a very small program for which the initialization routines make up the majority of the code, so being able to skip these is very beneficial.

I tested AFL++ in 2 modes, qemu and source code instrumented. Starting with qemu-mode and a -O3 compiled binary (without any snapshot/persistent fuzzing mechanisms enabled), AFL requires 2 and a half minutes to find the crash and runs at about 3500 fuzz cases per second. Taking the non-snapshot version of my fuzzer, this is a \~200x speedup. 

With source based instrumentation using the afl-clang-fast compiler with the flags shown below, AFL finds the crash in 3 minutes, and runs at 3500 fuzz cases per second once again. I would have expected this to run a lot faster than the qemu-based approach, but I believe that the setup should be correct. Things start to look a little different with persistent-mode/in memory fuzzing enabled. AFL++ is now able to generate 33,000 fuzz cases per second and also finds the crash in the first second.

`AFL_USE_ASAN=1 LLVM_CONFIG=llvm-config-11 ~/AFLplusplus/afl-clang-fast ../test_cases/simple_test.c -o simple_test_afl -O3`
`~/AFLplusplus/afl-fuzz (-D) -i in -o out -- ./simple_test_afl @@`

I believe that the emulated version of AFL using qemu is the fairer comparison since my fuzzer does not require source code for its instrumentation and fully emulates. I include both metrics though since my fuzzer currently only supports RISC-V which is not a very popular architecture yet, and will thus generally require source code as well to recompile to RISC-V.

#### Multi-threaded JIT Performance Against a Simple Target
**Experiment-Setup**  
This experiment used the exact same source code and test environment as the above test. The only difference is that both AFL++ and sfuzz were run in a multi-threaded manner. 16 threads were used for this test since that is what my machine supports. For sfuzz this setup consists of changing the NUM_THREADS variable in the `src/config.rs` file from 1 to 16.

**Results**

- <b>SFUZZ snapshot-based:</b> 13.5 million fuzz cases per second
- <b>SFUZZ no snapshot:</b> 6.4 million fuzz cases per second

The machine this has 8 cores with 2 threads each. Comparing these results to the previous single threaded case demonstrates that the fuzzer loses almost no performance when dealing with shared data structures for this simple case. This is especially highlighted because these very simple/small cases spend most of their time in overhead since the actual cases are super small. On the other hand, since the target is so small, there are not many accesses to eg. the shared coverage map, so future tests with more complex targets will be useful.

AFL++: TODO

#### Some More Complicated Targets

TODO