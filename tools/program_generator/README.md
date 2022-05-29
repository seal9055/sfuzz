# Program Generator

This tool automatically generates valid c code for the purposes of fuzzing. It includes functions,
arithmetic operations on provided input, various comparisons, potentially dangerous functions that
operate on provided values (eg. memcpy/strcpy), and it inserts crashes at random, deeply nested,
locations in the program.

All aspects of this can easily be configured, including the amount of different functions generated,
the depth of scope-blocks, and the size of the input and some allocated buffers.

20 million lines of dynamically generated lines of c-code can be generated in about 10 seconds.
(used configs for this: MAX_DEPTH=12 & NUM_FUNCTIONS = 4)
