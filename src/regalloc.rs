use crate::{
    irgraph::{Instruction, Reg},
    ssa_builder::{SSABuilder, Block},
    emulator::Register as PReg,
};

use std::collections::BTreeSet;
use rustc_hash::FxHashMap;
use iced_x86::Register::*;
use iced_x86::Register;

// 16 regs total, but only 10 useable
    // rcx = used as temporary scratch register
    // rsp = saved
    // r15 = Pointer to runtime-relevant pointers
        // r15 + 0x0  = memory-mapped registers
        // r15 + 0x8  = emulator memory
        // r15 + 0x10 = emulator permissions
        // r15 + 0x18 = emulator jit code-block lookup
//const PHYSREGSNUM: usize = 11;

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

    ///// The registers defined by φ-operations at entry of the block B
    //fn phi_defs(&self, block_index: usize) -> BTreeSet<Reg> {
    //    self.blocks[block_index].phi_funcs
    //        .iter()
    //        .map(|e| e.o_reg.unwrap())
    //        .collect::<BTreeSet<Reg>>()
    //}

    ///// The set of registers used in a φ-operation at entry of a block successor of the block B.
    //fn phi_uses(&self, block: &Block) -> BTreeSet<Reg> {
    //    let mut phi_uses = BTreeSet::new();

    //    for s in &block.succ {
    //        let j = self.blocks[*s].pred
    //            .iter()
    //            .position(|&x| x as usize == block.index)
    //            .unwrap();

    //        for i in &self.blocks[*s].phi_funcs {
    //            phi_uses.insert(i.i_reg[j]);
    //        }
    //    }
    //    phi_uses
    //}

    /// Start register allocation procedure. Involves liveness analysis, lifetime intervals,
    /// and ...
    pub fn get_reg_mapping(&mut self) -> FxHashMap<Reg, Register> {
        // Calculate live_in and live_out values for every block
        //self.liveness_analysis();

        self.number_instructions();

        // Compute liveness intervals for each register
        let mut intervals = self.build_intervals();

        for block in &self.blocks {
            block.phi_funcs.iter().for_each(|e| { println!("{}: {}", e.id, e); });
            block.instrs(&self.instrs).iter().for_each(|e| { println!("{}: {}", e.id, e); });
        }

        let reg_mappings = self.linear_scan(&mut intervals);

        reg_mappings
    }

    /// Number all instructions in block, making sure predecessors are numbered first
    fn number_instructions(&mut self) {
        let mut numbered_blocks = vec![0usize; self.blocks.len()];
        let mut cur_count: isize = 0;

        // Make sure to number all blocks
        for block in self.blocks.clone() {
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
            num(&block);

            for i in 0..block.phi_funcs.len() {
                self.blocks[block.index].phi_funcs[i].id = block.instrs(&self.instrs)[0].id;
            }
        }
    }

    /// Determines how long each register is alive
    fn build_intervals(&mut self) -> FxHashMap<Reg, (isize, isize)> {
        let mut intervals: FxHashMap<Reg, (isize, isize)> = FxHashMap::default();

        // TODO May want to get reverse blocks here for better regalloc
        for block in &self.blocks {

            for phi in &block.phi_funcs {
                intervals.insert(phi.o_reg.unwrap(), (phi.id, phi.id));
            }

            for instr in block.instrs(&self.instrs) {
                if instr.o_reg.is_some() {
                    if intervals.get(&instr.o_reg.unwrap()).is_none() {
                        intervals.insert(instr.o_reg.unwrap(), (instr.id, instr.id));
                    } else {
                        intervals.get_mut(&instr.o_reg.unwrap()).unwrap().0 = instr.id;
                    }
                }

                for input in instr.i_reg {
                    if intervals.get(&input).is_none() {
                        intervals.insert(input, (33, instr.id));
                    } else if instr.id > intervals.get(&input).unwrap().1 {
                        intervals.get_mut(&input).unwrap().1 = instr.id;
                    }
                }
            }

            for phi in &block.phi_funcs {
                for input in &phi.i_reg {
                    if intervals.get(&input).is_none() {
                        intervals.insert(*input, (33, phi.id));
                    } else if phi.id > intervals.get(&input).unwrap().1 {
                        intervals.get_mut(&input).unwrap().1 = phi.id;
                    }
                }
            }
        }

        intervals
    }

    /// Simple Linear scan register allocation algorithm
    fn linear_scan(&mut self, tmp: &mut FxHashMap<Reg, (isize, isize)>) 
        -> FxHashMap<Reg, Register> {
        let mut intervals: Vec<(Reg, (isize, isize))> = Vec::new();
        let mut count = 0;

        // Very hacky way of sorting this, fix it later
        while count <= self.instrs.len() {
            for v in tmp.iter() {
                if v.1.0 == count as isize {
                    intervals.push((*v.0, *v.1));
                }
            }
            count += 1;
        }

        let mut mapping: FxHashMap<Reg, Register> = FxHashMap::default();
        let mut free_regs: Vec<Register> 
            = vec![RAX, RBX, RDX, RSI, RDI, R8, R9, R10, R11, R12, R13, R14];
        let mut active: FxHashMap<Register, (isize, isize)> = FxHashMap::default();

        for i in intervals {
            let reg   = i.0;
            let inter = i.1;

            /* expire old */
            for v in active.clone().iter() {
                if v.1.1 >= inter.0 { continue; }
                active.remove(&v.0);
                free_regs.push(*v.0);
            }

            // Hardcode stack pointer to rbp
            if reg.0 == PReg::Sp {
                mapping.insert(reg, RBP);
                continue;
            }

            if free_regs.len() == 0 {
                // Spill a register to memory
                mapping.insert(reg, None);
            } else {
                let preg = free_regs.pop().unwrap();
                active.insert(preg, inter);
                mapping.insert(reg, preg);
            }
        }
        mapping
    }

    /*
       Algorithm
            Starting from a register use, traverse the CFG backwards until the registers definition
            is reached. While traversing this path, add the register to the traversed blocks'
            live_in and live_out sets as appropriate.

            Computing Liveness Sets for SSA-Form Programs - Brandner et al.
    */
    ///// Traverse blocks and determine live_in and live_out registers for each block
    //fn liveness_analysis(&mut self) {
    //    for block in &mut self.blocks.clone() {
    //        for v in self.phi_uses(block) {
    //            self.blocks[block.index].live_out.insert(v);
    //            self.up_and_mark(block, v);
    //        }

    //        for instr in block.instrs(&self.instrs) {
    //            instr.i_reg.iter().for_each(|e| { self.up_and_mark(block, *e); });
    //        }
    //    }
    //}

    ///// Perform the path exploration initialized by liveness_analysis()
    //fn up_and_mark(&mut self, block: &mut Block, v: Reg) -> bool {
    //    // Killed in the block
    //    if block
    //        .instrs(&self.instrs)
    //        .iter()
    //        .filter_map(|e| e.o_reg)
    //        .collect::<BTreeSet<Reg>>()
    //        .contains(&v) {
    //            return false;
    //        }

    //    // Propagation already completed, kill
    //    if block.live_in.contains(&v) { return false; }

    //    // The conditional is dependant on if phi function definitions cound as live-in
    //    if !block.phi_funcs
    //        .iter()
    //        .map(|e| e.o_reg.unwrap())
    //        .any(|e| e == v) {
    //            self.blocks[block.index].live_in.insert(v);
    //        }

    //    // Do not propagate phi-definitions
    //    if self.phi_defs(block.index).contains(&v) { return false; }

    //    let mut pred_blocks: Vec<Block> = block.pred
    //        .iter()
    //        .map(|e| self.blocks[*e].clone())
    //        .collect();

    //    for pred in &mut pred_blocks {
    //        self.blocks[pred.index].live_out.insert(v);
    //        if !self.up_and_mark(pred, v) { return false; }
    //    }
    //    true
    //}
}
