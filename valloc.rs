use crate::{
    irgraph::{Instruction, Reg, Operation},
    ssa_builder::{SSABuilder, Block},
    emulator::{NUMREGS, Register},
};

use std::collections::BTreeSet;
use rustc_hash::FxHashMap;

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
        // Calculate live_in and live_out values for every block
        self.liveness_analysis();

        // Compute liveness intervals for each register
        self.gen_moves();

        for instr in &self.instrs {
            println!("{}: {}", instr.id, instr);
        }

        panic!("panic hit in regalloc");
    }

    fn gen_moves(&mut self) {
        for block in &self.blocks {
            for i in &block.pred {
                let pred = &self.blocks[*i];

                let mut n_block: Option<usize> = None;

                if block.pred.len() > 1 && pred.succ.len() > 1 {
                    // Insert new block
                } else {
                    // n = p
                }

                for phi_func in &block.phi_funcs {
                    for input in &phi_func.i_reg {
                        if input.block() != pred {
                            continue;
                        }
                        let copy = Instruction {
                            op:    Operation::Mov,
                            o_reg: Some(Reg(phi_func.o_reg.unwrap().0, 
                                            phi_func.o_reg.unwrap().1+1)),
                            i_reg: vec![*input],
                            flags: 0,
                            id: 0,
                            pc: None,
                        };

                        // mutate original operand

                    }
                }
            }
        }
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

        // The conditional is dependant on if phi function definitions cound as live-in
        if !block.phi_funcs
            .iter()
            .map(|e| e.o_reg.unwrap())
            .any(|e| e == v) {
                self.blocks[block.index].live_in.insert(v);
            }

        // Do not propagate phi-definitions
        if self.phi_defs(block.index).contains(&v) { return false; }

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
