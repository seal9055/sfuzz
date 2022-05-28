/// Trait to return random value from an enum
trait Randomness {
    fn get_rand_op(remaining_depth: usize) -> Self;
}

#[derive(Debug, Clone)]
enum Value {
    Number(u64),
    StringLiteral(String),
    True,
    False,
}

#[derive(Debug, Clone)]
enum Type {
    Void
}

/// Operations that can occur in the code
#[derive(Debug, Clone)]
enum Operation {
    AddInts(usize, usize),
    SubInts(usize, usize),
    If(Expr, Block),

    // All operations below this point should not be returned by the `get_rand_op()` function, and
    // are solely used for special cases such as program initialization or inserting crashes
    
    /// Used in `main` to allocate the input buffer based on argv
    AllocBuf,

    /// Used in `main` to call generated functions
    CallFunc(Function),
}

impl Randomness for Operation {
    /// Return a random operation
    fn get_rand_op(remaining_depth: usize) -> Self {
        let num_entries = std::mem::variant_count::<Operation>();
        let (r1, r2) = RNG.get2_rand();
        match RNG.next_num(num_entries)  {
            0 => Operation::AddInts(r1, r2),
            1 => Operation::SubInts(r1, r2),
            2 => Operation::If(Expr::get_rand_op(remaining_depth), 
                               Block::init_new_block(remaining_depth - 1)),
            _ => Operation::AddInts(r1, r2),
        }
    }
}
#[derive(Debug, Clone)]
enum Expr {
    /// Index into input-array and 8-bit value
    ByteCmp(usize, u8), 

    /// Index into input-array and 16-bit value
    WordCmp(usize, u16), 

    /// Index into input-array and 32-bit value
    DWordCmp(usize, u32), 

    /// Index into input-array and 64-bit value to be used for comparison operation
    QWordCmp(usize, u64), 
}

impl Randomness for Expr {
    /// Return a random Expression
    fn get_rand_op(_remaining_depth: usize) -> Self {
        let num_entries = std::mem::variant_count::<Expr>();
        let (r1, r2) = RNG.get2_rand();
        match RNG.next_num(num_entries) {
            0 => Expr::ByteCmp(r1 % INPUT_SIZE, r2 as u8),
            1 => Expr::WordCmp(r1 % INPUT_SIZE, r2 as u16),
            2 => Expr::DWordCmp(r1 % INPUT_SIZE, r2 as u32),
            3 => Expr::QWordCmp(r1 % INPUT_SIZE, r2 as u64),
            _ => unreachable!(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Stmt {
    /// Default expressions
    Expression(Expr),

    /// Name, return type, arguments, body
    Function(String, Type, Vec<Expr>, Block)
}
