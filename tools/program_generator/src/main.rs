
use program_generator::{
    Program, compile
};

fn main() {
    let program = Program::create_program();
    compile(program);
}
