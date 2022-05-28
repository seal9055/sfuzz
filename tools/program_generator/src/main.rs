use program_generator::{
    compile::compile,
    Program, 
};

fn main() {
    let program = Program::create_program();
    compile(program);
}
