

    ; edge = hash(to, from)
    ; edge_track ^= new_edge

    ; update from

    mov al, 0
    cmp al, 1
    je fallthrough

    ; overwrite 0 -> 1
    ; Track coverage code
    ; exit_jit (reentry, code)

.fallthrough
    ; continue execution


block hit
    -> cur_hash = from ^ to
    -> hash ^= cur_hash
    -> From = pc
