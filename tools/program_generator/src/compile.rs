use crate::{
    Program, Type, Block, Operation
};

/// Prints out the generated program to stdout
const DEBUG_PRINT: bool = true;

/// Can be set to false to not actually write cases to disk & compile while debugging
const WRITE_TO_DISK: bool = true;

/// Compiler used to compile the c-code once generated
const COMPILER: &str = "/opt/riscv/bin/riscv64-unknown-elf-gcc";

/// Contains information that the compiler functions require while generating the c code from the
/// intermediate representation
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
    /// Create a new compiler object
    pub fn new(program: Program) -> Self {
        Self {
            cur_depth: 0,
            code: String::new(),
            program,
        }
    }

    /// Debug print for the c-code
    pub fn print_code(&self) {
        println!("Generated the following code:");
        println!("+-------------------------------------------------+");
        println!("{}", self.code);
        println!("+-------------------------------------------------+");
    }

    /// Insert indentation into the code based on the current depth
    fn insert_indent(&mut self) {
        for _ in 0..self.cur_depth {
            self.code.push_str("    ");
        }
    }

    /// Setup headers and begin actual program translation
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


    /// Translate the header of a function to c
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

    /// Emit an operation to the c-code
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

    /// Translate an entire block to c-code while taking care of handling proper scoping and
    /// indentation
    fn translate_block(&mut self, block: &Block) {
        self.insert_indent();
        self.code.push_str("{\n");
        self.cur_depth += 1;

        for operation in &block.stmt_list {
            self.emit_operation(operation);
        }

        self.cur_depth -= 1;
        self.insert_indent();
        self.code.push_str("}\n\n");
    }

    /// Translate the body of a function
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

        // Compile the generated program
        std::process::Command::new(COMPILER)
            .arg("generated_program.c")
            .arg("-o")
            .arg("generated_program")
            .spawn()
            .expect("Failed to compile generated program");
    }

    println!("[+] Done generating code, if the program is rather large, your compiler might still \
             be busy compiling though");
}
