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
const INPUT_SIZE: usize = 5000;

/// Maximum depth that scopes can go too before early returning. Without this blocks would
/// recursively create new blocks until a stack overflow occurs
const MAX_DEPTH: usize = 4;

/// Determines the amount of functions that are created outside of `main`
const NUM_FUNCTIONS: usize = 2;

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
            Type::Str => write!(f, "char *"),
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
        let signs = ["==", "<", "<=", ">", ">="];
        let s = signs[RNG.next_num(signs.len())];
        match self {
            Expr::VarByteCmp(a, b) |
            Expr::VarWordCmp(a, b) |
            Expr::VarDWordCmp(a, b) |
            Expr::VarQWordCmp(a, b) |
            Expr::VarStrCmp(a, b) => {
                write!(f, "buf[{}] == {}", a, b)
            },
            Expr::ByteCmp(a, b) => write!(f, "buf[{}] == {}", a, b),
            Expr::WordCmp(a, b) => write!(f, "(unsigned) (atol(buf + {}) & 0xffff) {} {}", a, s, b),
            Expr::DWordCmp(a, b) => write!(f, "(unsigned) atol(buf + {}) {} {}", a, s, b),
            Expr::QWordCmp(a, b) => write!(f, "(unsigned) atoll(buf + {}) {} {}ULL", a, s, b),
            Expr::StrCmp(a, b) => write!(f, "strcmp(&buf[{}], {})", a, b),
        }
    }
}

const NUM_SIMPLE_OPS: usize = 2;
const NUM_COMPLEX_OPS: usize = 2;

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

    /// Memcpy operation into a previously allocated buffer
    MemCpy(String, Index, String),

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

    /// Used in `main` to call generated functions
    CallFunc(String, Type, Vec<Type>),

    /// Allocate a new buffer on the function stack, (var-name, size)
    AllocStackBuf(String, usize),

    /// Allocate a new buffer on the heap, (var-name, size)
    AllocHeapBuf(String, usize),

    /// Insert a crash
    Crash,
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Operation::AddInts(a, b, c, d) => write!(f, "{} {} = buf[{}] + {}", a, b, c, d),
            Operation::SubInts(a, b, c, d) => write!(f, "{} {} = buf[{}] - {}", a, b, c, d),
            Operation::If(a, _) => write!(f, "if ({}) ", a),
            Operation::AllocInputBuf => write!(f, "buf = malloc({})", INPUT_SIZE),
            Operation::ArgvCheck => write!(f, "if (argc != 2) return"),
            Operation::OpenFile => write!(f, "FILE *fd = fopen(argv[1], \"r\")"),
            Operation::ReadFile => write!(f, "fgets(buf, {}, fd)", INPUT_SIZE),
            Operation::Crash => write!(f, "*(unsigned long*)0x{:x} = 0", RNG.gen()),
            Operation::AllocStackBuf(a, b) => write!(f, "char {}[{}]", a, b),
            Operation::AllocHeapBuf(a, b) => write!(f, "char *{} = malloc({})", a, b),
            Operation::MemCpy(a, b, c) => write!(f, "memcpy({}, buf+{}, {})", a, b, c),
            Operation::CallFunc(a, _, _) => write!(f, "{}()", a),
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
    fn get_complex_op(remaining_depth: usize, vars: &Vec<(String, Type)>) -> Self {
        let _var_name = std::str::from_utf8(&RNG.next_string(16, 0x41, 0x7b)).unwrap().to_string();

        let buf_vars = vars.iter().filter(|e| e.1 == Type::Buffer)
            .map(|e| e.0.clone()).collect::<Vec<String>>();

        let num_vars = vars.iter().filter(|e| e.1 == Type::Number)
            .map(|e| e.0.clone()).collect::<Vec<String>>();

        loop {
            match RNG.next_num(NUM_COMPLEX_OPS)  {
                0 => { 
                    return Operation::If(Expr::get_rand_expr(vars), 
                                   Block::init_new_block(remaining_depth - 1));
                },
                1 => { 
                    if buf_vars.is_empty() || num_vars.is_empty() { continue; }
                    return Operation::MemCpy(
                        buf_vars[RNG.next_num(buf_vars.len())].clone(), 
                        Index(RNG.next_num(INPUT_SIZE-MAX_ALLOC_SIZE)),
                        num_vars[RNG.next_num(num_vars.len())].clone(), 
                        );
                },
                _ => unreachable!(),
            }
        }
    }
}

/// Scoped block with allocated variables and a list of statements to be executed
#[derive(Debug, Default, Clone)]
pub struct Block {
    stmt_list: Vec<Operation>,

    /// (Name, Type)
    variables: Vec<(String, Type)>,
}

impl Block {
    /// Create a new block initialized with random operations
    pub fn init_new_block(remaining_depth: usize) -> Self {
        let mut block = Block::default();

        // Insert a crash and early return if maximum depth has been reached
        if remaining_depth == 0 {
            block.stmt_list.push(Operation::Crash);
            return block;
        }

        // Allocate some buffers for this block that can be used in future operations
        let var_name1 = std::str::from_utf8(&RNG.next_string(16, 0x61, 0x7b)).unwrap().to_string();
        let var_name2 = std::str::from_utf8(&RNG.next_string(16, 0x61, 0x7b)).unwrap().to_string();
        let size = RNG.gen_range(MIN_ALLOC_SIZE, MAX_ALLOC_SIZE);
        block.stmt_list.push(Operation::AllocStackBuf(var_name1.clone(), size));
        block.stmt_list.push(Operation::AllocHeapBuf(var_name2.clone(), size));
        block.variables.push((var_name1, Type::Buffer));
        block.variables.push((var_name2, Type::Buffer));

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
            let op = Operation::get_complex_op(remaining_depth, &block.variables);
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

impl Function {
    pub fn new(name: &str, remaining_depth: usize) -> Self {
        Function {
            name: name.to_string(),
            typ:  Type::Void,
            arguments: Vec::new(),
            body: Block::init_new_block(remaining_depth)
        }
    }

    /// Create main function. It has a special case since it requires additional initialization 
    /// routines
    pub fn create_main(functions: &[Function]) -> Self {
        Function {
            name: "main".to_string(),
            typ:  Type::Void,
            arguments: vec![(Type::Number, "argc".to_string()), (Type::Argv, "argv".to_string())],
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

        // Create random generated functions that can be called from main
        for i in 0..NUM_FUNCTIONS {
            let func_name = format!("func_{}", i+1);
            program.function_list.push(Function::new(&func_name, MAX_DEPTH));
        }

        // Create main function
        program.function_list.push(Function::create_main(&program.function_list));

        program
    }
}

