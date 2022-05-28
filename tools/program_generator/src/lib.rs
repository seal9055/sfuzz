#![feature(variant_count)]
#![feature(once_cell)]

pub mod rand;
pub mod types;

use rand::Rand;
use types::{Stmt, Expr};

use std::lazy::SyncLazy;

/// This program takes an input file via argv[1], this variable specifies the amount of bytes that
/// are read in and available for use from the input, larger values should make finding the bugs a
/// little harder
const INPUT_SIZE: usize = 100;

/// Maximum depth that scopes can go too before early returning. Without this blocks would
/// recursively create new blocks until a stack overflow occurs
const MAX_DEPTH: usize = 5;

/// Create an rng object on program startup
pub static RNG: SyncLazy<Rand> = SyncLazy::new(|| {
    Rand::new()
});


/// Scoped block
#[derive(Debug, Default, Clone)]
struct Block {
    action_list: Vec<Operation>,
}

impl Block {
    /// Create a new block initialized with random operations
    pub fn init_new_block(remaining_depth: usize) -> Self {
        let mut block = Block::default();

        // Early return if maximum depth has been reached
        if remaining_depth == 0 {
            return block;
        }

        for _ in 0..(5 + RNG.next_num(5)) {
            block.action_list.push(Operation::get_rand_op(remaining_depth));
        }
        block
    }

    pub fn init_main_block(functions: &[Function]) -> Self {
        let mut block = Block::default();
        block.action_list.push(Operation::AllocBuf);

        for func in functions {
            block.action_list.push(Operation::CallFunc(func.clone()));
        }

        block
    }
}

#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    typ: Type,
    arguments: Vec<Value>,
    body: Block,
}

impl Function {
    pub fn new(name: &str, remaining_depth: usize) -> Self {
        Function {
            name: name.to_string(),
            typ:  Type::Void,
            arguments: Vec::new(),
            body: Block::init_new_block(remaining_depth)
        }
    }

    /// Main function is a special case since it needs to setup initialization routines
    pub fn create_main(functions: &[Function]) -> Self {
        Function {
            name: "main".to_string(),
            typ:  Type::Void,
            arguments: Vec::new(),
            body: Block::init_main_block(functions), 
        }
    }
}

/// The actual program being modelled
#[derive(Debug, Default, Clone)]
pub struct Program {
    function_list: Vec<Function>,
}

impl Program {
    /// Start creation of the program
    pub fn create_program() -> Program {
        let mut program = Program::default();

        // Create 1-5 random functions that can be called from main
        for i in 0..RNG.gen_range(1, 2) {
            let func_name = format!("func_{}", i+1);
            program.function_list.push(Function::new(&func_name, MAX_DEPTH));
        }

        // Create main function
        program.add_function(Function::create_main(&program.function_list));

        program
    }

    pub fn add_function(&mut self, func: Function) {
        self.function_list.push(func);
    }
}


/// Compile the previously generated program to an elf binary
pub fn compile(program: Program) {
    println!("Received the following program: {:#?}", program);
    // Start by loading argv[1] into global variable called buf
}

