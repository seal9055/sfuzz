use crate::{
    irgraph::{Instruction, Reg},
    ssa_builder::{SSABuilder, Block},
    emulator::Register,
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

        self.number_instructions();

        // Compute liveness intervals for each register
        let intervals = self.build_intervals();

        for instr in &self.instrs {
            println!("{}: {}", instr.id, instr);
        }
        for v in intervals {
            println!("{:?}: {:?}", v.0, v.1);
        }

        panic!("panic hit in regalloc");
    }

    /// Number all instructions in block, making sure predecessors are numbered first
    fn number_instructions(&mut self) {
        let mut numbered_blocks = vec![0usize; self.blocks.len()];
        let mut cur_count: isize = 0;

        // Make sure to number all blocks
        for block in &self.blocks {
            // Given a block that has not yet been numbered, this closure numbers all instructions 
            // in the block
            let mut num = |b: &Block| {
                // Already numbered this block
                if numbered_blocks[b.index] == 1 { return; }

                // Number all instructions in a block
                for i in b.start..=b.end {
                    self.instrs[i].id = cur_count;
                    cur_count += 1;
                }

                // Block has now been numbered
                numbered_blocks[b.index] = 1;
            };

            // Make sure to number all predecessor instructions first
            block.pred
                .iter()
                .map(|e| self.blocks[*e].clone())
                .for_each(|e| num(&e));
            num(block);
        }
    }

    fn edit_range(map: &mut FxHashMap<Reg, Vec<(usize, usize)>>, reg: Reg, 
                  from: Option<usize>, to: Option<usize>) {

        if map.get(&reg).is_none() {
            map.insert(reg, Vec::new());
        } else {
            map.get_mut(&reg).unwrap().push((from.unwrap_or(77), to.unwrap_or(66)));
        }
        //    if matches!(reg, Reg(Register::A1, 1)) {
        //        println!("DBG B4: {}: [{:?}]", reg, map.get(&reg).unwrap());
        //    }
        //    if from.is_some() && from.unwrap() < map.get(&reg).unwrap().0 {
        //        map.get_mut(&reg).unwrap().0 = from.unwrap();
        //    }
        //    if to.is_some() && to.unwrap() > map.get(&reg).unwrap().1 {
        //        map.get_mut(&reg).unwrap().1 = to.unwrap();
        //    }
        //}
        //if matches!(reg, Reg(Register::A1, 1)) {
        //    println!("DBG AF: {}: [{:?}]", reg, map.get(&reg).unwrap());
        //}
    }

    /* Inputs:
        1. Instructions in ssa form
        2. Linear block order with all of a block's dominators being located before the block
       Output:
        One lifetime interval for each virtual register (can contain lifetime holes)
    */
    /// Constructs lifetime intervals for blocks
    fn build_intervals(&mut self) -> FxHashMap<Reg, Vec<(usize, usize)>> {
        let mut intervals: FxHashMap<Reg, Vec<(usize, usize)>> = FxHashMap::default();

        let rev_blocks = vec![4, 2, 3, 1, 0];

        for b in rev_blocks {
            let block = self.blocks[b].clone();
            // 0. Add all live_in registers of block's successors to the current live set
            let mut live: BTreeSet<Reg> = BTreeSet::new();
            for s in &block.succ {
                self.blocks[*s].live_in
                    .iter()
                    .for_each(|e| { live.insert(*e); });
            }

            // 1. Phi-function inputs of succeeding functions pertaining to Block are live
            for s in &block.succ {
                self.blocks[*s].phi_funcs
                    .iter()
                    .for_each(|phi| { 
                        if block.index == 1 {
                            live.insert(phi.i_reg[0]); 
                        } else if block.index == 3 {
                            live.insert(phi.i_reg[1]); 
                        }
                    }); // TODO cant hardcode
            }


            // 2. Update live interval for each register in live-in
            for reg in &live {
                Regalloc::edit_range(&mut intervals, *reg, Some(block.start), Some(block.end));
            }

            // Remove def's from live and add inputs to live
            // Also update intervals
            for instr in &block.rev_instrs(&self.instrs) {
                if instr.o_reg.is_some() {
                    Regalloc::edit_range(&mut intervals, instr.o_reg.unwrap(), 
                                         Some(instr.id as usize), None);

                    live.remove(&instr.o_reg.unwrap());
                }

                for input in &instr.i_reg {
                    Regalloc::edit_range(&mut intervals, *input, Some(block.start), 
                                         Some(instr.id as usize));
                    live.insert(*input);
                }
            }

            // Remove out_regs from live sets
            for phi_func in &block.phi_funcs {
                live.remove(&phi_func.o_reg.unwrap());
            }

            // TODO Handle loops

            // May be unnecessary
            self.blocks[block.index].live_in = live;
        }

        return intervals;
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
