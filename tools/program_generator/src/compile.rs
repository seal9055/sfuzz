use crate::{
    Program, Type, Block, Operation
};

/// Prints out the generated program to stdout
const DEBUG_PRINT: bool = true;

/// Can be set to false to not actually write cases to disk & compile while debugging
const WRITE_TO_DISK: bool = false;

#[derive(Default, Debug)]
pub struct Compiler {
    /// Scope depth, used to handle indentation
    cur_depth: usize,

    /// The actual c code that will be compiled
    code: String,

    /// The program being generated in an intermediate representation
    program: Program,
}

impl Compiler {
    pub fn new(program: Program) -> Self {
        Self {
            cur_depth: 0,
            code: String::new(),
            program,
        }
    }

    pub fn print_code(&self) {
        println!("Generated the following code:");
        println!("+-------------------------------------------------+");
        println!("{}", self.code);
        println!("+-------------------------------------------------+");
    }

    fn insert_indent(&mut self) {
        for _ in 0..self.cur_depth {
            self.code.push_str("    ");
        }
    }

    pub fn translate_program(&mut self) {
        self.code.push_str("#include <stdio.h>\n");
        self.code.push_str("#include <stdlib.h>\n");
        self.code.push_str("#include <string.h>\n\n");
        self.code.push_str("char *buf;\n\n");

        for i in 0..self.program.function_list.len() {
            self.translate_function_header(i);
            self.translate_function_body(i);
        }
    }


    fn translate_function_header(&mut self, index: usize) {
        let mut first = true;
        let func = self.program.function_list[index].clone();
        self.code.push_str(&format!("{} {}(", func.typ, func.name));
        for arg in func.arguments {
            if first {
                first = false;
            } else {
                self.code.push_str(", ");
            }
            if arg.0 == Type::Argv {
                self.code.push_str(&format!("char **{}", arg.1));
            } else {
                self.code.push_str(&format!("{} {}", arg.0, arg.1));
            }
            
        }
        self.code.push_str(")\n");
    }

    fn emit_operation(&mut self, operation: &Operation) {
        self.insert_indent();
        match operation {
            Operation::If(_, b) => {
                self.code.push_str(&format!("{}\n", operation));
                self.translate_block(b);
            },
            _ => self.code.push_str(&format!("{};\n", operation)),
        };
    }

    fn translate_block(&mut self, block: &Block) {
        self.insert_indent();
        self.code.push_str("{\n");
        self.cur_depth += 1;

        for operation in &block.action_list {
            self.emit_operation(operation);
        }

        self.cur_depth -= 1;
        self.insert_indent();
        self.code.push_str("}\n\n");
    }

    fn translate_function_body(&mut self, index: usize) {
        let body = self.program.function_list[index].body.clone();
        self.translate_block(&body);
    }
}

/// Compile the previously generated program to an elf binary
pub fn compile(program: Program) {
    if DEBUG_PRINT {
        println!("Received the following program: \n{:#?}", program);
    }

    let mut compiler = Compiler::new(program);
    compiler.translate_program();

    if DEBUG_PRINT {
        compiler.print_code();
    }

    if WRITE_TO_DISK {
        // Write the program to disk
        std::fs::write("generated_program.c", &compiler.code)
            .expect("Failed to write generated program to disk");

        std::process::Command::new("gcc")
            .arg("generated_program.c")
            .arg("-o")
            .arg("generated_program")
            .spawn()
            .expect("Failed to compile generated program");
    }
}
