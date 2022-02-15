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
            edges: ssa.edges.clone(),
            live_in: Vec::new(),
            live_out: Vec::new(),
        }
    }

    /// The registers defined by φ-operations at entry of the block B
    fn phi_defs(&self, block_index: usize) -> BTreeSet<Reg> {
        self.blocks[block_index].phi_funcs
            .iter()
            .map(|e| e.o_reg.unwrap())
            .collect::<BTreeSet<Reg>>()
    }

    /// The set of registers used in a φ-operation at entry of a block successor of the block B.
    fn phi_uses(&self, block: &Block) -> BTreeSet<Reg> {
        let mut phi_uses = BTreeSet::new();

        for s in &block.succ {
            let j = self.blocks[*s].pred
                .iter()
                .position(|&x| x as usize == block.index)
                .unwrap();

            for i in &self.blocks[*s].phi_funcs {
                phi_uses.insert(i.i_reg[j]);
            }
        }
        phi_uses
    }

    /// Start register allocation procedure. Involves liveness analysis, lifetime intervals,
    /// and ...
    pub fn execute(&mut self) {
        // 1. Compute liveness intervals
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

        //self.blocks.iter().for_each(|e| { println!("{:#?}", e); });

        let mut intervals: Vec<(usize, usize)> = Vec::new();
        let rev_blocks = self.blocks.iter().rev();

        for block in rev_blocks {
            // Live-in of Block successors need to be alive in Block
            let live: Vec<Reg> = Vec::new();
            block.succ.iter().for_each(|e| live.push(self.blocks[*e].live_in));

            // phi-function inputs of succeeding functions pertaining to Block also need to be live
            for phi_func in self.blocks[block.succ].phi_funcs {
                live.push(phi_func.i_regs[block]);
            }

            // Update live interval for each register in live
            for reg in live {
                intervals[reg] = (block.start, block.end);
            }

            // remove def's from live and add inputs to live
            // Also update intervals
            for instr in block.rev_instrs(&self.instrs) {
                intervals[instr.o_reg].0 = cur_instr_index;
                live.remove[instr.o_reg];

                for input in instr.i_reg {
                    intervals[input] = (block_start, cur_instr_index);
                    live.add(input);
                }
            }

            // Remove out_regs from live sets
            for phi_func in block {
                live.remove(phi_func.o_reg);
            }

            /*
            // TODO later
            if block.is_loop_header() {
                loop_end = // last block of loop
                for reg in live {
                    intervals[reg] = (block_start, loopEnd)
                }
            }
            */

            // May be unnecessary
            block.livein = live;
        }

        return intervals;


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
        for block in &mut self.blocks.clone() {
            for v in self.phi_uses(block) {
                self.blocks[block.index].live_out.insert(v);
                self.up_and_mark(block, v);
            }

            for instr in block.instrs(&self.instrs) {
                instr.i_reg.iter().for_each(|e| { self.up_and_mark(block, *e); });
            }
        }
    }

    /// Perform the path exploration initialized by liveness_analysis()
    fn up_and_mark(&mut self, block: &mut Block, v: Reg) -> bool {
        // Killed in the block
        if block
            .instrs(&self.instrs)
            .iter()
            .filter_map(|e| e.o_reg)
            .collect::<BTreeSet<Reg>>()
            .contains(&v) {
                return false;
            }

        // Propagation already completed, kill
        if block.live_in.contains(&v) { return false; }

        self.blocks[block.index].live_in.insert(v);

        // Do not propagate phi-definitions
        if self.phi_defs(block.index).contains(&v) { return false; }

        // TODO fix the clone
        let mut pred_blocks: Vec<Block> = block.pred
            .iter()
            .map(|e| self.blocks[*e].clone())
            .collect();

        for pred in &mut pred_blocks {
            self.blocks[pred.index].live_out.insert(v);
            if !self.up_and_mark(pred, v) { return false; }
        }
        true
    }
}
