#![feature(variant_count)]
#![feature(once_cell)]

pub mod rng;
pub mod compile;

use rng::Rng;

use std::fmt;
use std::lazy::SyncLazy;

/// This program takes an input file via argv[1], this variable specifies the amount of bytes that
/// are read in and available for use from the input, larger values should make finding the bugs a
/// little harder
const INPUT_SIZE: usize = 500;

/// Maximum depth that scopes can go too before early returning. Without this blocks would
/// recursively create new blocks until a stack overflow occurs. Recommended: 8-12 for approximately 
/// 2,000 - 200,000 lines of code. For larger complexity scores, the INPUT_SIZE should also be
/// increased to reduce duplication
const COMPLEXITY: usize = 8;

/// Minimum depth of functions, prevents too shallow functions that just immediately crash on base
/// case
const MIN_DEPTH: usize = 1;

/// Minimum and maximum sizes for buffer allocations in the program.
const MIN_ALLOC_SIZE: usize = 0x20;
const MAX_ALLOC_SIZE: usize = 0x100;

/// Maximum length for strings that can be used in comparisons. This needs to be smaller than
/// `INPUT_SIZE`
const MAX_STRING_LEN: usize = 0x20;

/// Index into the provided user input
#[derive(Debug, Clone, Copy)]
pub struct Index(usize);

impl fmt::Display for Index {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Create an rng object on program startup
pub static RNG: SyncLazy<Rng> = SyncLazy::new(|| {
    Rng::new()
});

/// Supported values
#[derive(Debug, Clone)]
pub enum Value {
    Number(usize),
    StringLiteral(String),
    Arr(Vec<Value>),
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Value::Number(v) => write!(f, "{}", v),
            Value::StringLiteral(v) => write!(f, "\"{}\"", v),
            _ => unreachable!(),
        }
    }
}

/// Supported types
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Type {
    Void,
    Number,
    Str,
    Argv,
    Buffer,
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Type::Void => write!(f, "void"),
            Type::Number => write!(f, "int"),
            Type::Str => write!(f, "unsigned char *"),
            Type::Buffer => write!(f, "unsigned char*"),
            _ => unreachable!(),
        }
    }
}

/// Expressions that are used in if-statements
#[derive(Debug, Clone)]
enum Expr {
    /// Index into input-array and 8-bit value
    ByteCmp(Index, u8), 

    /// Index into input-array and variable containing an 8-bit Value
    VarByteCmp(Index, String), 

    /// Index into input-array and 16-bit value
    WordCmp(Index, u16), 

    /// Index into input-array and variable containing a 16-bit Value
    VarWordCmp(Index, String), 

    /// Index into input-array and 32-bit value
    DWordCmp(Index, u32), 

    /// Index into input-array and variable containing a 32-bit Value
    VarDWordCmp(Index, String), 

    /// Index into input-array and 64-bit value to be used for comparison operation
    QWordCmp(Index, u64), 

    /// Index into input-array and variable containing a 64-bit Value
    VarQWordCmp(Index, String), 

    /// Index into input-array and a ByteString used for comparison operation
    StrCmp(Index, Value), 

    /// Index into input-array and a ByteString used for comparison operation with a variable
    VarStrCmp(Index, String), 
}

impl Expr {
    /// Return a random Expression
    fn get_rand_expr(vars: &Vec<(String, Type)>) -> Self {
        let num_entries = std::mem::variant_count::<Expr>();
        let rstr = std::str::from_utf8(&RNG.next_string(16, 0x61, 0x7b)).unwrap().to_string();
        let rnum = RNG.gen();

        let num_vars = vars.iter().filter(|e| e.1 == Type::Number)
            .map(|e| e.0.clone()).collect::<Vec<String>>();

        let str_vars = vars.iter().filter(|e| e.1 == Type::Str)
            .map(|e| e.0.clone()).collect::<Vec<String>>();

        loop {
            match RNG.next_num(num_entries) {
                0 => {
                    return Expr::ByteCmp(Index(RNG.next_num(INPUT_SIZE)), rnum as u8);
                },
                1 => {
                    if num_vars.is_empty() { continue; }
                    return Expr::VarByteCmp(Index(RNG.next_num(INPUT_SIZE)), 
                                            num_vars[RNG.next_num(num_vars.len())].clone());
                },
                2 => {
                    return Expr::WordCmp(Index(RNG.next_num(INPUT_SIZE)), rnum as u16);
                },
                3 => {
                    if num_vars.is_empty() { continue; }
                    return Expr::VarWordCmp(Index(RNG.next_num(INPUT_SIZE)), 
                                            num_vars[RNG.next_num(num_vars.len())].clone());
                },
                4 => {
                    return Expr::DWordCmp(Index(RNG.next_num(INPUT_SIZE)), rnum as u32);
                },
                5 => {
                    if num_vars.is_empty() { continue; }
                    return Expr::VarDWordCmp(Index(RNG.next_num(INPUT_SIZE)), 
                                            num_vars[RNG.next_num(num_vars.len())].clone());
                },
                6 => {
                    return Expr::QWordCmp(Index(RNG.next_num(INPUT_SIZE)), rnum as u64);
                },
                7 => {
                    if num_vars.is_empty() { continue; }
                    return Expr::VarQWordCmp(Index(RNG.next_num(INPUT_SIZE)), 
                                            num_vars[RNG.next_num(num_vars.len())].clone());
                },
                8 => {
                    return Expr::StrCmp(Index(RNG.next_num(INPUT_SIZE-MAX_STRING_LEN)), 
                                  Value::StringLiteral(rstr));
                }
                9 => {
                    if str_vars.is_empty() { continue; }
                    return Expr::VarStrCmp(Index(RNG.next_num(INPUT_SIZE-MAX_STRING_LEN)), 
                                    str_vars[RNG.next_num(str_vars.len())].clone());
                }
                _ => unreachable!(),
            };
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let signs = ["=="];
        let s = signs[RNG.next_num(signs.len())];
        match self {
            Expr::VarByteCmp(a, b) |
            Expr::VarWordCmp(a, b) |
            Expr::VarDWordCmp(a, b) |
            Expr::VarQWordCmp(a, b) |
            Expr::VarStrCmp(a, b) => {
                write!(f, "buf[{}] == {}", a, b)
            },
            Expr::ByteCmp(a, b) => write!(f, "buf[{}] {} {}", a, s, b),
            Expr::WordCmp(a, b) => write!(f, "*(unsigned short*)(buf + {}) {} {}", a, s, b),
            Expr::QWordCmp(a, b) => write!(f, "*(unsigned int*)(buf + {}) {} {}U", a, s, b),
            Expr::DWordCmp(a, b) => write!(f, "*(unsigned long*)(buf + {}) {} {}ULL", a, s, b),
            Expr::StrCmp(a, b) => write!(f, "!strcmp(&buf[{}], {})", a, b),
        }
    }
}

const NUM_SIMPLE_OPS: usize = 2;
const NUM_COMPLEX_OPS: usize = 1;

/// Operations that can occur in the code
#[derive(Debug, Clone)]
enum Operation {
    // Simple Operations
    // These are operations that occur at the start of a block, and solely exist to setup some
    // random local variables that can then later be used my some more complex operations
    
    /// Add input[.0] to .1 and assign it to a variable
    AddInts(Type, String, Index, usize),

    /// Subtract input[.0] from .1 and assign it to a variable
    SubInts(Type, String, Index, usize),

    // Complex Operations
    // These are operations that occur at the start of a block, and solely exist to setup some
    // random local variables that can then later be used my some more complex operations

    /// If expression alongside a true-block
    If(Expr, Block),

    /// Used to call generated functions (name, type, args)
    CallFunc(String, Type, Vec<Type>),

    /// Insert a crash
    Crash,

    // All operations below this point should not be returned by the `get_rand_op()` function, and
    // are solely used for special cases such as program initialization or inserting crashes
    
    /// Used in `main` to allocate the input buffer based on argv
    AllocInputBuf,

    /// Used to check that argv was properly provided in main
    ArgvCheck,

    /// Used to open the file provided by argv in main
    OpenFile,

    /// Used to read in the fuzz-input from the provided file
    ReadFile,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Operation::AddInts(a, b, c, d) => write!(f, "{} {} = buf[{}] + {}", a, b, c, d),
            Operation::SubInts(a, b, c, d) => write!(f, "{} {} = buf[{}] - {}", a, b, c, d),
            Operation::If(a, _) => write!(f, "if ({}) ", a),
            Operation::AllocInputBuf => write!(f, "unsigned char *buf = malloc({})", INPUT_SIZE),
            Operation::ArgvCheck => write!(f, "if (argc != 2) return"),
            Operation::OpenFile => write!(f, "FILE *fd = fopen(argv[1], \"r\")"),
            Operation::ReadFile => write!(f, "fgets(buf, {}, fd)", INPUT_SIZE),
            Operation::Crash => write!(f, "*(unsigned long*)0x{:x} = 0", RNG.gen()),
            Operation::CallFunc(a, _, _) => write!(f, "{}(buf)", a),
        }
    }
}

impl Operation {
    /// Return a random simple operation
    fn get_simple_op() -> Self {
        let var_name = std::str::from_utf8(&RNG.next_string(16, 0x61, 0x7b)).unwrap().to_string();

        match RNG.next_num(NUM_SIMPLE_OPS)  {
            0 => Operation::AddInts(Type::Number, var_name,
                    Index(RNG.next_num(INPUT_SIZE)), RNG.gen_range(MIN_ALLOC_SIZE, MAX_ALLOC_SIZE)),
            1 => Operation::SubInts(Type::Number, var_name,
                    Index(RNG.next_num(INPUT_SIZE)), RNG.gen_range(MIN_ALLOC_SIZE, MAX_ALLOC_SIZE)),
            _ => unreachable!(),
        }
    }

    /// Return a random more complex operation
    fn get_complex_op(program: &mut Program, vars: &Vec<(String, Type)>, complexity: usize, 
                      depth: usize) -> Self {
        let _var_name = std::str::from_utf8(&RNG.next_string(16, 0x41, 0x7b)).unwrap().to_string();

        loop {
            match RNG.next_num(NUM_COMPLEX_OPS)  {
                0 => { 
                    return Operation::If(Expr::get_rand_expr(vars), 
                                   Block::init_new_block(program, complexity - 1, depth + 1));
                },
                _ => unreachable!(),
            }
        }
    }
}

/// Scoped block with allocated variables and a list of statements to be executed
#[derive(Debug, Default, Clone)]
pub struct Block {
    /// Statements contained in a block
    stmt_list: Vec<Operation>,

    /// (Name, Type)
    variables: Vec<(String, Type)>,
}

impl Block {
    /// Create a new block initialized with random operations
    pub fn init_new_block(program: &mut Program, complexity: usize, depth: usize) -> Self {
        let mut block = Block::default();

        // If the minimum depth has been reached, there's a chance that the block will be terminated
        // on a crash or by calling a different function
        if depth >= MIN_DEPTH {
            let num = RNG.gen_range(0, complexity);
            if num < 5 {
                if num < 2 {
                    // Insert crash
                    block.stmt_list.push(Operation::Crash);
                } else {
                    // Insert function call
                    let func = program.function_list.get(RNG.next_num(program.function_list.len()));
                    
                    // Insert a function-call unless 'main' was retrieved, or no functions exist
                    // yet, in which case just insert a crash
                    if let Some(f) = func {
                        if f.name == "main" {
                            block.stmt_list.push(Operation::Crash);
                        } else {
                            block.stmt_list.push(
                                Operation::CallFunc(
                                    f.name.clone(),
                                    Type::Void,
                                    Vec::new(),
                            ));
                        }
                    } else {
                        block.stmt_list.push(Operation::Crash);
                    }
                }

                return block;
            }
        }

        // Start by inserting some simple operations to setup some variables that can later be used
        for _ in 0..RNG.gen_range(2, 5) {
            let op = Operation::get_simple_op();

            // If this operation produces a value, add it to this blocks variables
            match &op {
                Operation::AddInts(typ, name, ..) |
                Operation::SubInts(typ, name, ..) => {
                    block.variables.push((name.clone(), *typ));
                },
                _ => {},
            }
            block.stmt_list.push(op);
        }

        // Next insert some more complex operations
        for _ in 0..RNG.gen_range(5, 10) {
            let op = Operation::get_complex_op(program, &block.variables, complexity, depth);
            block.stmt_list.push(op);
        }
        block
    }

    /// Create the main block. This just handles initial setup and calls the functions that should
    /// be fuzzed
    pub fn init_main_block(functions: &[Function]) -> Self {
        let mut block = Block::default();

        // Allocate a global buffer to hold argv and write fuzz-input it
        block.stmt_list.push(Operation::ArgvCheck);
        block.stmt_list.push(Operation::OpenFile);
        block.stmt_list.push(Operation::AllocInputBuf);
        block.stmt_list.push(Operation::ReadFile);

        // Create a call to all functions
        for func in functions {
            block.stmt_list.push(Operation::CallFunc(
                        func.name.clone(),
                        func.typ,
                        func.arguments.iter().map(|e| e.0).collect(),
                    ));
        }
        block
    }
}

/// Intermediate representation of functions
#[derive(Debug, Clone)]
pub struct Function {
    name: String,
    typ: Type,
    arguments: Vec<(Type, String)>,
    body: Block,
}

/// The actual program being modelled
#[derive(Debug, Default, Clone)]
pub struct Program {
    /// List of generated functions
    function_list: Vec<Function>,
}

impl Program {
    pub fn default() -> Self {
        Self {
            function_list: Vec::new(),
        }
    }

    /// Start creation of the program
    pub fn create_program() -> Program {
        let mut program = Program::default();

        // Create random generated functions that can be called from main
        for i in 0..COMPLEXITY {
            let func_name = format!("func_{}", i+1);
            let func = Function {
                    name: func_name.to_string(),
                    typ:  Type::Void,
                    arguments: vec![(Type::Buffer, "buf".to_string())],
                    body: Block::init_new_block(&mut program.clone(), COMPLEXITY, 0)
                };
            program.function_list.push(func);
        }

        // Create main function
        program.create_main();

        program
    }

    /// Create main function. It has a special case since it requires additional initialization 
    /// routines
    fn create_main(&mut self) {
        self.function_list.push(
            Function {
                name: "main".to_string(),
                typ:  Type::Void,
                arguments: vec![(Type::Number, "argc".to_string()), (Type::Argv, "argv".to_string())],
                body: Block::init_main_block(&self.function_list), 
            });
    }
}

