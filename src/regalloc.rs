use crate::{
    irgraph::{Instruction, Reg},
    ssa_builder::{SSABuilder, Block},
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
    pub blocks: Vec<Block>,

    /// Calculated phi nodes
    pub phi_func: Vec<BTreeSet<(Reg, isize)>>,

    /// Registers that are alive coming into a block corresponding to the vector index
    pub live_in: Vec<BTreeSet<Reg>>,

    /// Registers that are alive coming out of a block corresponding to the vector index
    pub live_out: Vec<BTreeSet<Reg>>,
}

impl Regalloc {
    pub fn new(ssa: &SSABuilder) -> Self {
        Regalloc {
            instrs: ssa.instrs.clone(),
            idom_tree: ssa.idom_tree.clone(),
            blocks: ssa.blocks.clone(),
            phi_func: ssa.phi_func.clone(),
            edges: ssa.edges.clone(),
            live_in: Vec::new(),
            live_out: Vec::new(),
        }
    }

    /// The registers defined by φ-operations at entry of the block B
    fn phi_defs(&self, block_index: usize) -> BTreeSet<Reg> {
        self.blocks[block_index]
            .instrs(&self.instrs)
            .iter()
            .filter(|e| e.is_phi_function())
            .map(|e| e.o_reg.unwrap())
            .collect::<BTreeSet<Reg>>()
    }

    /// The set of registers used in a φ-operation at entry of a block successor of the block B.
    fn phi_uses(&self, block_index: usize) -> BTreeSet<Reg> {
        let mut phi_uses = BTreeSet::new();

        let succ_blocks: Vec<Block> = self.edges
            .iter()
            .filter(|v| v.0 == block_index as u32)
            .map(|e| self.blocks[e.1 as usize])
            .collect();

        for block in succ_blocks {
            let pred: Vec<u32> = self.edges.iter()
                .filter(|v| v.1 == block.1 as u32).map(|e| e.0).collect();
            let j = pred.iter().position(|&x| x as usize == block_index).unwrap();

            for i in block.instrs(&self.instrs).iter().filter(|e| e.is_phi_function()) {
                phi_uses.insert(i.i_reg[j]);
            }
        }
        phi_uses
    }

    /// Start register allocation procedure. Involves liveness analysis, lifetime intervals,
    /// and ...
    pub fn execute(&mut self) {

        // 1. Compute liveness and global next uses
        self.build_intervals();

        panic!("panic hit in regalloc");
    }

    /// Temporary debug prints
    fn print_debug(&self) {
        println!("\n{{");
        for v in &self.live_out {
            println!("live_out: {:?}", v);
        }
        for v in &self.live_in {
            println!("live_in: {:?}", v);
        }
        println!("}}\n");
    }

    /* Inputs:
        1. Instructions in ssa form
        2. Linear block order with all of a block's dominators being located before the block
       Output:
        One lifetime interval for each virtual register (can contain lifetime holes)
    */
    /// Constructs lifetime intervals for blocks
    fn build_intervals(&mut self) {

        // Calculate correct live_in and live_out values for every block
        self.liveness_analysis();

        self.print_debug();

        panic!("Done with liveness analysis");
    }

    /*
       Algorithm
            Starting from a register use, traverse the CFG backwards until the registers definition
            is reached. While traversing this path, add the register to the traversed blocks'
            live_in and live_out sets as appropriate.

            Computing Liveness Sets for SSA-Form Programs - Brandner et al.
    */
    /// Traverse blocks and determine live_in and live_out registers for each block
    fn liveness_analysis(&mut self) {
        // Initialize empty live_out & live_in sets for all registers
        for _ in 0..self.blocks.len()-1 {
            self.live_out.push(BTreeSet::new());
            self.live_in.push(BTreeSet::new());
        }

        for block in &self.blocks.clone() {
            for v in self.phi_uses(block.1) {
                self.live_out[block.1].insert(v);
                self.up_and_mark(block.1, v);
            }

            for instr in block.instrs(&self.instrs).iter().filter(|e| !e.is_phi_function()) {
                instr.i_reg.iter().for_each(|e| { self.up_and_mark(block.1, *e); });
            }
        }
    }

    /// Perform the path exploration initialized by liveness_analysis()
    fn up_and_mark(&mut self, block_index: usize, v: Reg) -> bool {
        // Killed in the block
        let block_defs = self.blocks[block_index]
            .instrs(&self.instrs)
            .iter()
            .filter(|e| !e.is_phi_function())
            .filter_map(|e| e.o_reg)
            .collect::<BTreeSet<Reg>>();
        if block_defs.contains(&v) { return false; }

        // Propagation already completed, kill
        if self.live_in[block_index].contains(&v) { return false; }

        self.live_in[block_index].insert(v);

        // Do not propagate phi-definitions
        if self.phi_defs(block_index).contains(&v) { return false; }

        // Propagate backwards
        let predecessors: Vec<u32> = self.edges
            .iter()
            .filter(|v| v.1 == block_index as u32)
            .map(|e| e.0)
            .collect();

        for pred in predecessors {
            self.live_out[pred as usize].insert(v);
            println!("#2 ({})Inserting into live_out[{}]: {}", block_index, pred, v);
            if !self.up_and_mark(pred as usize, v) { return false; }
        }
        true
    }
}
