use crate::{
    irgraph::{Operation, IRGraph, Instruction, Reg},
    emulator::{Register as PReg, NUMREGS},
    ssa_builder::SSABuilder,
};

use std::collections::BTreeSet;

#[derive(Debug, Default)]
pub struct Regalloc {
    /// List of all instructions in a given graph
    pub instrs: Vec<Instruction>,

    /// Map the index of a block in the blocks array to the index of the block its jumping to
    pub edges: Vec<(u32, u32)>,

    /// Contains the immediate dominator (idom) for a given node (node 1 has none)
    pub idom_tree: Vec<(isize, isize)>,

    /// Basic blocks in the program
    pub blocks: Vec<((usize, usize), usize)>,

    /// Calculated phi nodes
    pub phi_func: Vec<BTreeSet<(Reg, isize)>>,
}

impl Regalloc {
    // TODO might be able to move in object and del clones
    pub fn new(ssa: &SSABuilder) -> Self {
        println!("idom: {:?}", &ssa.idom_tree);
        println!("blocks: {:?}", &ssa.blocks);
        Regalloc {
            instrs: ssa.instrs.clone(),
            idom_tree: ssa.idom_tree.clone(),
            blocks: ssa.blocks.clone(),
            phi_func: ssa.phi_func.clone(),
            edges: ssa.edges.clone(),
        }
    }

    pub fn execute(&mut self) {

        self.build_intervals();

        panic!("panic hit in regalloc");
    }

    /* Inputs:
        1. Instructions in ssa form
        2. Linear block order with all of a block's dominators being located before the block
       Output:
        One lifetime interval for each virtual register (can contain lifetime holes)
    */
    /// Constructs lifetime intervals for blocks
    fn build_intervals(&mut self) {
        let mut rev_blocks = self.blocks.clone();
        rev_blocks.reverse();

        for block in &rev_blocks {
            let cur_block = block.1;

            // 1. Setup a vector `live` of livein regs for each successor of b
                // union of all registers live at the beginning of the successors of b

            // 2. For each phi_func in b's successors `live.push(phi_input(b))`
            let successors: Vec<u32> = self.edges.iter()
                .filter(|v| v.0 == cur_block as u32).map(|e| e.1).collect();

            for s in &successors {
                let succ_block = self.blocks[*s as usize];

                // loop through phi-functions
                for i in succ_block.0.0+1..succ_block.0.1 {
                    let instr = &mut self.instrs[i];
                    if instr.op != Operation::Phi { break; }
                    // live.push(phi.inputOf(b))
                }
            }

            // 3. For each def in live, add a live-range from block_start to block_end for def-reg

            // 4. For instr in b, set intervals for both defs and uses

            // 5. For each phi_func of b, remove phi.o_reg from live

            // 6. Handle loops
        }
    }
}
