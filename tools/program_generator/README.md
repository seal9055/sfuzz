# Program Generator

This tool automatically generates valid c code for the purposes of fuzzing. It includes functions,
arithmetic operations on provided input, various comparisons, potentially dangerous functions that
operate on provided values (eg. memcpy/strcpy), and it inserts crashes at random, deeply nested,
locations in the program.

Use the globals in lib.rs and compile.rs to specify the desired complexity of the generated code,
and which compiler should be used to generate the code.
