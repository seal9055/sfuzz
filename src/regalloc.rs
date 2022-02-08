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
        let cur_block = self.blocks[block_index];
        let cur_block_instrs = &self.instrs.clone()[cur_block.0.0..cur_block.0.1];
        cur_block_instrs
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
            let irs = &self.instrs[block.0.0..block.0.1];

            for i in irs.iter().filter(|e| e.is_phi_function()) {
                i.i_reg.iter().for_each(|e| { phi_uses.insert(*e); });
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

        println!("\n{{");
        for v in &self.live_out {
            println!("live_out: {:?}", v);
        }
        for v in &self.live_in {
            println!("live_in: {:?}", v);
        }
        println!("}}\n");

        panic!("Done with liveness analysis");
    }

    fn liveness_analysis(&mut self) {
        // Initialize empty live_out & live_in sets for all registers
        for _ in 0..self.blocks.len()-1 {
            self.live_out.push(BTreeSet::new());
            self.live_in.push(BTreeSet::new());
        }

        let blocks = self.blocks.clone();
        for block in &blocks {
            for v in self.phi_uses(block.1) {
                self.live_out[block.1].insert(v);
                self.up_and_mark(block.1, v);
            }

            for instr in self.instrs.clone()[block.0.0..block.0.1]
                    .iter()
                    .filter(|e| !e.is_phi_function()) {
                        // TODO for v in ins.reads()
                instr.i_reg.iter().for_each(|e| { self.up_and_mark(block.1, *e); });
            }
        }
    }

    fn up_and_mark(&mut self, block_index: usize, v: Reg) -> bool {
        let cur_block = self.blocks[block_index];
        let cur_block_instrs = &self.instrs.clone()[cur_block.0.0..cur_block.0.1];

        // TODO may be an issue with ssa form

        // Killed in the block
        let block_defs = cur_block_instrs
            .iter()
            .filter(|e| !e.is_phi_function())
            .filter(|e| e.o_reg.is_some())
            .map(|e| e.o_reg.unwrap())
            .collect::<BTreeSet<Reg>>();
        if block_defs.contains(&v) { println!("hit 1"); return false; }

        // Propagation already completed, kill
        if self.live_in[block_index].contains(&v) { println!("hit 2"); return false; }

        self.live_in[block_index].insert(v);

        // Do not propagate phi-definitions
        if self.phi_defs(block_index).contains(&v) { println!("hit 3"); return false; }

        // Propagate backwards
        let pred: Vec<u32> = self.edges.iter()
                .filter(|v| v.1 == block_index as u32).map(|e| e.0).collect();
        for p in pred {
            self.live_out[block_index].insert(v);
            if !self.up_and_mark(p as usize, v) { return false; }
        }
        return true;
    }
}
