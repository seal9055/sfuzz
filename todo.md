### High Priority

1. Need to change rbp referencing instructions since rbp currently points to unmapped memory. (Only
   occurs when ssa form is completely destructed)

2. JIT bug when compiling 0x10400, some range miscalculation resulting in jump to wrong address

3. Figure out how to handle function transitions

### Low Priority

1. Add some light optimizations onto ssa form to improve final codegen

2. Replace naive live interval calculations with a more mature algorithm
