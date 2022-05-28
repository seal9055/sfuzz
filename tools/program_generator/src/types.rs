#[derive(Debug, Clone)]
enum Value {
    Number(u64),
    StringLiteral(String),
    True,
    False,
}

#[derive(Debug, Clone)]
enum Type {
    Void, 
    Bool,
}

/// Operations that can occur in the code
#[derive(Debug, Clone)]
enum Expr {
    AddInts(usize, usize),
    SubInts(usize, usize),

    // These enums are used for comparison operations in if/while statements. 
    // (Index into input-array, value to compare against)
    ByteCmp(usize, u8), 
    WordCmp(usize, u16), 
    DWordCmp(usize, u32), 
    QWordCmp(usize, u64), 

    // All operations below this point should not be returned by the `get_rand_op()` function, and
    // are solely used for special cases such as program initialization or inserting crashes
    
    /// Used in `main` to allocate the input buffer based on argv
    AllocBuf,

    /// Used in `main` to call generated functions
    FunctionCall {
        callee: Box<Expr>,
        typ: Type,
        arguments: Vec<Expr>,
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    /// Default expressions
    Expression(Expr),

    /// Name, return type, arguments, body
    Function(String, Value, Vec<Expr>, Vec<Stmt>),

    /// Vector of statements that make up the actual block
    Block(Vec<Stmt>),

    /// expression + true and (optionally) false blocks
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
}
