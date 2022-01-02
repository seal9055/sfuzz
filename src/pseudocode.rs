#[derive(Clone)]
pub struct Emu {
    pub memory: Mmu,

    pub state: State,
}

impl Emu {

    pub fn run_emu(&mut self) {

        loop {
            pc = self.get_reg(Register::Pc);

            let instr = memory_at(pc);

            if instr.is_fun() {
                let fun_jit_cache = JIT::get_addr(pc);

                if fun_jit_cache == -1 {
                    compile_jit;
                    let fun_jit_cache = JIT::get_addr(pc);
                }

                // start JIT procedure for function
            }
        }
    }

}
