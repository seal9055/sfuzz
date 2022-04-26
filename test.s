


    mov rbx, <str_addr>
    lea rax, [rbx + 1]

.loop_start
    mov cl, byte [rax]
    inc rax
    test cl, cl
    jnz loop_start

    sub rax, rbx
    ret
    
