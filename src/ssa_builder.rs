use crate::{
    irgraph::{Operation, IRGraph, Instruction, Reg},
    emulator::{Register as PReg, NUMREGS},
};

use std::fs::File;
use std::io::Write;
use std::collections::BTreeSet;

use petgraph::Graph;
use petgraph::dot::{Dot, Config};
use rustc_hash::FxHashMap;

/// Struct used to represent blocks of code in the program according to the CFG representation
#[derive(Debug, Default, Clone)]
pub struct Block {
    /// Index of the block in CFG
    pub index:     usize,

    /// First instruction in the block
    pub start:     usize,

    /// Final instruction in the block
    pub end:       usize,

    /// Label of the current block (indicates start pc)
    pub label:     usize,

    /// Phi functions added to the block
    pub phi_funcs: Vec<Instruction>,

    /// Registers alive at start of block (used for regalloc)
    pub live_in:   BTreeSet<Reg>,

    /// Registers alive at end of block (used for regalloc)
    pub live_out:  BTreeSet<Reg>,

    /// CFG successor of this block
    pub succ:      Vec<usize>,

    /// CFG predecessors of this block
    pub pred:      Vec<usize>,
}

impl Block {
    /// Return new block with given start, end & index
    pub fn new(start: usize, end: usize, index: usize) -> Self {
        Self {
            index,
            start,
            end,
            label:     0,
            phi_funcs: Vec::new(),
            live_in:   BTreeSet::new(),
            live_out:  BTreeSet::new(),
            succ:      Vec::new(),
            pred:      Vec::new(),
        }
    }

    /// The instructions used in a block
    pub fn instrs(&self, instrs: &[Instruction]) -> Vec<Instruction> {
        instrs[self.start..=self.end].to_vec()
    }

    /// The instructions used in a block in reverse (used by regalloc)
    pub fn rev_instrs(&self, instrs: &[Instruction]) -> Vec<Instruction> {
        let mut rev = Vec::new();
        for i in (self.start..=self.end).rev() {
            rev.push(instrs[i].clone());
        }
        rev.reverse();
        rev
    }

    /// The set of registers used in a given block
    pub fn regs_used(&self, instrs: &[Instruction]) -> Vec<Reg> {
        self.instrs(instrs)
            .iter()
            .flat_map(|e| &e.i_reg)
            .copied()
            .collect::<Vec<Reg>>()
    }

    /// The set of registers defined in a given block
    pub fn regs_def(&self, instrs: &[Instruction]) -> Vec<Reg> {
        self.instrs(instrs)
            .iter()
            .filter_map(|e| e.o_reg)
            .collect::<Vec<Reg>>()
    }
}

/// Struct that has a lot of helper fields that are used during ssa construction
#[derive(Debug, Default)]
pub struct SSABuilder {
    /// Map the index of a block in the blocks array to the index of the block its jumping to
    pub edges: Vec<(u32, u32)>,

    /// List of all instructions in a given graph
    pub instrs: Vec<Instruction>,

    /// Set that marks the first instruction of each block
    pub leader_set: Vec<(Instruction, usize)>,

    /// Contains the immediate dominator (idom) for a given node (node 1 has none)
    pub idom_tree: Vec<(isize, isize)>,

    /// List of all variables alongside their index in the instruction array (potential duplicates)
    pub var_origin: Vec<(Reg, usize)>,

    /// Dominance frontier of a given block
    pub dominance_frontier: Vec<BTreeSet<isize>>,

    /// Basic blocks in the program
    pub blocks: Vec<Block>,

    /// Count/Stack used for register renaming
    pub reg_stack: Vec<(usize, Vec<usize>)>,
}

impl SSABuilder {
    /// Loop through the instructions of a given function and generate the control flow graph for it
    pub fn new(irgraph: &IRGraph) -> SSABuilder {
        let mut ssa_builder   = SSABuilder::default();
        let mut index: isize = -1;
        let mut iterator = irgraph.instrs.iter().peekable();
        let mut map: FxHashMap<usize, isize> = FxHashMap::default();
        let mut edges = Vec::new();
        let mut tmp_labels = FxHashMap::default();
        let mut i = 0;

        // Determine labels locations
        while let Some(instr) = &iterator.next() {
            // Label indicates new block start
            if instr.pc.is_some() && irgraph.labels.get(&instr.pc.unwrap()).is_some() {
                index += 1;
                map.insert(instr.pc.unwrap(), index);
                tmp_labels.insert(index, instr.pc.unwrap());
                ssa_builder.leader_set.push(((*instr).clone(), i));
            }
            match instr.op {
                Operation::Branch(x, y) => { /* End basic block with a branch to 2 other blocks */
                    edges.push((index as u32, y as u32));
                    edges.push((index as u32, x as u32));
                }
                Operation::Jmp(x) => { /* End basic block with a non-returning jmp */
                    edges.push((index as u32, x as u32));
                },
                _ => { }
            }
            i+=1;
        }

        // Set appropriate edges for control flow graph
        for edge in edges {
            let v = *(map.get(&(edge.1 as usize)).unwrap()) as u32;
            ssa_builder.edges.push((edge.0, v));
        }

        ssa_builder.reg_stack  = vec![(0, Vec::new()); NUMREGS];
        ssa_builder.instrs = irgraph.instrs.clone();

        // Initiate blocks, these track the first and last instruction for each block
        ssa_builder.leader_set.push((Instruction::default(), ssa_builder.instrs.len()));
        for (i, v) in ssa_builder.leader_set.iter().enumerate() {
            ssa_builder.blocks.push(Block::new(v.1, ssa_builder.leader_set[i+1].1-1, i));
            if i == ssa_builder.leader_set.len()-2 { break; }
        }

        // Initialize labels
        for block in &mut ssa_builder.blocks {
            block.label = *tmp_labels.get(&(block.index as isize)).unwrap();
        }

        // Initialize predecessors and successors for each block
        for block in &mut ssa_builder.blocks {
            (block.pred, block.succ) = ssa_builder.edges
                .iter()
                .fold((Vec::new(), Vec::new()), |mut acc, e| {
                    if e.1 == block.index as u32 { acc.0.push(e.0 as usize); }
                    if e.0 == block.index as u32 { acc.1.push(e.1 as usize); }
                    acc
                });
        }

        ssa_builder
    }

    /// Convert ir representation to single static assignment form by calling multiple helpers
    pub fn build_ssa(&mut self) {
        let mut dom_tree;
        let var_list;
        let varlist_origin;
        let var_tuple;

        (self.idom_tree, dom_tree) = self.generate_domtree();
        self.dominance_frontier = self.find_domfrontier(&mut dom_tree);
        (var_list, self.var_origin, varlist_origin, var_tuple) = self.find_var_origin();
        self.add_phi_funcs(&varlist_origin, &var_tuple);
        self.rename(&var_list);
    }

    /*
        Algorithm:
            For every block in the graph call the dom closure. This function loops through all
            previous blocks that lead to the current block and adds them to the idom and dom tree's
            as appropriate.
    */
    /// Generate dominator tree, this tracks both an entire list of dominators for a given node, and
    /// its immediate dominator.
    fn generate_domtree(&mut self) -> (Vec<(isize, isize)>, Vec<BTreeSet<isize>>) {
        let initial: isize = self.edges[0].0 as isize;
        let mut dom_temp: Vec<BTreeSet<isize>> = Vec::new();

        let num_leaders = self.leader_set.len() - 1;

        for i in 0..num_leaders {
            dom_temp.push(btreeset![initial, i as isize]);
        }

        let mut dom = move |n: usize| {
            let mut dom_set = dom_temp[n].clone();
            let mut dom_check: Vec<BTreeSet<isize>> = Vec::new();

            self.blocks[n].pred
                .iter()
                .for_each(|e| { dom_check.push(dom_temp[*e as usize].clone()); });

            let dom_inter = &dom_check[0];
            let dom_inter = dom_check.iter().fold(BTreeSet::new(), |_, e| {
                e.intersection(dom_inter).collect()
            });

            dom_inter.iter().for_each(|e| { dom_set.insert(**e); });
            dom_set.iter().for_each(|e| { dom_temp[n].insert(*e); });
            dom_set
        };

        let mut idom_tree: Vec<(isize, isize)> = Vec::new();
        let mut dom_tree: Vec<BTreeSet<isize>> = Vec::new();

        for i in 1..num_leaders {
            let mut dom_tempset: BTreeSet<isize> = dom(i);

            dom_tree.push(dom_tempset.clone());
            dom_tempset.remove(&(i as isize));
            let max_val = dom_tempset.into_iter().max().unwrap();
            idom_tree.push((max_val as isize, i as isize));
        }

        (idom_tree, dom_tree)
    }

    /*
        Algorithm:
            Locate all join points j
            For each predecessor p of j, walk up the dominator tree from p until a node is found
            that dominates j. All nodes in this traversal from p to the node that dominates j, but
            not including j belong to the dominance frontier.
    */
    /// Find Dominance Frontiers
    fn find_domfrontier(&mut self, dom_tree: &mut Vec<BTreeSet<isize>>) -> Vec<BTreeSet<isize>> {

        // Add an extra node at the beginning of the graph that dominates everything.
        // This makes the implementation a little simpler
        dom_tree.insert(0, btreeset!{0});
        self.idom_tree.insert(0, (-1, 0));

        // Create an index in the dominance frontier set for every node in the graph
        let mut dominance_frontier: Vec<BTreeSet<isize>> = vec![BTreeSet::new(); dom_tree.len()];

        for v in &self.idom_tree {
            let join_point: usize = v.1 as usize;

            // Loop through all predecessors of the join point. If a predecessor is not an idom,
            // insert it into the dominance frontier set
            for p in &self.blocks[join_point].pred {
                let mut runner: isize = *p as isize;

                while runner != self.idom_tree[join_point as usize].0 {
                    dominance_frontier[runner as usize].insert(join_point as isize);
                    runner = self.idom_tree[runner as usize].0;
                }
            }
        }
        dominance_frontier
    }

    /*
       Returns a couple of structures describing def/use relationships between registers and their
       corresponding blocks. These are useful to simplify the algorithms in future functions,
       although a lot of this can most likely be removed during a future refactor.
    */
    /// Returns a couple different register mappings that will be useful later
    fn find_var_origin(&self)
        -> (Vec<Reg>, Vec<(Reg, usize)>, Vec<Vec<Reg>>, Vec<(Reg, usize)>) {

        let mut var_origin = Vec::new();

        //// Extract all register definitions from the function
        for (i, instr) in self.instrs.iter().enumerate() {
            if instr.o_reg.is_some() && instr.o_reg.unwrap().0 != PReg::Zero {
                var_origin.push((instr.o_reg.unwrap(), i));
            }
        }

        let leader_set_index: Vec<usize> = self.leader_set.iter().map(|e| e.1).collect();
        //leader_set_index.push(self.instrs.len());

        let mut varnode_origin: Vec<usize> = Vec::new();
        let mut i = 0;

        for x in &var_origin {
            let instr_index = x.1;
            while instr_index >= leader_set_index[i+1] { i += 1; }
            varnode_origin.push(i);
        }

        let mut varlist_temp: Vec<(Reg, usize)> = Vec::new();
        for i in 0..var_origin.len() {
            varlist_temp.push((var_origin[i].0, varnode_origin[i]));
        }

        let mut varlist_origin: Vec<Vec<Reg>> = Vec::new();
        for v in &varlist_temp {
            while varlist_origin.len() <= v.1 {
                let tmp: Vec<Reg> = Vec::new();
                varlist_origin.push(tmp);
            }
            varlist_origin[v.1].push(v.0);
        }

        let var_list = var_origin.iter().map(|v| v.0).collect();

        /* var_origin may be incorrect because block sizes change from phifuncs */
        (var_list, var_origin, varlist_origin, varlist_temp)
    }

    /*
        Algorithm:
            Whenever a register x is defined in a block b, a phi function needs to be inserted at
            the start of every dominance frontier of b. Since every phi function insertion may lead
            to more phi functions being inserted, we need to loop through all potential register
            definitions after every insertion.
    */
    /// Determine which nodes require phi functions and for which registers
    fn add_phi_funcs(&mut self, varlist_origin: &[Vec<Reg>], var_tuple: &[(Reg, usize)]) {

        let mut def_sites: Vec<Vec<usize>>      = vec![Vec::new(); NUMREGS];
        let mut var_phi:   Vec<BTreeSet<usize>> = Vec::new();

        // Vector of all registers, each index contains a vector that lists all blocks in which its
        // register was declared
        for v in var_tuple {
            def_sites[v.0.0 as usize].push(v.1);
            var_phi.push(BTreeSet::new());
        }

        let mut count = 0;
        for (i, var) in def_sites.iter().enumerate() {
            if var.is_empty() { continue; }
            count += 1;

            // worklist of blocks
            let mut worklist = def_sites[i].clone();

            while let Some(block) = worklist.pop() {
                for x in &self.dominance_frontier[block] {

                    if !var_phi[count].contains(&(*x as usize)) {
                        // If the block has no phi functions for x, insert phi functions
                        self.blocks[*x as usize].phi_funcs.push( Instruction {
                            op: Operation::Phi,
                            i_reg: Vec::new(),
                            o_reg: Some(Reg(PReg::from(i as u32), 0)),
                            flags: 0,
                            pc: None,
                            id: 0,
                        });

                        var_phi[count].insert(*x as usize);

                        // if x is not in varlist_origin, update the worklist
                        if !varlist_origin[block].iter().any(|&e| e ==
                                                             Reg(PReg::from(*x as u32), 0)) {
                            worklist.push(*x as usize);
                        }
                    }
                }
            }
        }
    }

    /*
        Algotihm:
            Start by setting up a count and a stack for each individual register.
                The count is used to track the newest ssa variant of the register
                The stack is used to track the ssa index that is currently in use for each register
           Finally get the basic blocks and call the rename_block function using the first block
    */
    /// Initiate procedure to start naming registers
    fn rename(&mut self, var_list: &[Reg]) {
        // Initialize the current positions for all used registers in this function to 0
        for var in var_list {
            self.reg_stack[var.0 as usize] = (0, vec![0; 1]);
        }
        self.rename_block(0);
    }

    /*
        Algotihm:
            1. Rename the output register of all phi functions of the current block.
            2. Loop through all instructions of the current block and rename inputs and outputs
            3. Go through all successors of the current block and set their phi function input regs
            4. Recursively call this function for each of its successors
            5. Destroy the register stack that this function created
    */
    /// Used as as a part of the rename procedure to be recursively called
    fn rename_block(&mut self, block_num: usize) {
        // Rename any existing phi functions at the start of the function
        for instr in &mut self.blocks[block_num].phi_funcs {
                // Increase count and push new count onto the stack
                self.reg_stack[instr.o_reg.unwrap().0 as usize].0 += 1;
                let count = self.reg_stack[instr.o_reg.unwrap().0 as usize].0;
                self.reg_stack[instr.o_reg.unwrap().0 as usize].1.push(count);

                let cur_reg = instr.o_reg.unwrap().0;
                instr.o_reg = Some(Reg(cur_reg, *self.reg_stack[cur_reg as usize].1
                                       .last().unwrap() as u16));
        }

        // Rename inputs and outputs
        for i in self.blocks[block_num].start..self.blocks[block_num].end {
            let instr = &mut self.instrs[i];

            // Rename the input registers
            for i in 0..instr.i_reg.len() {
                if instr.i_reg[i].0 == PReg::Zero { continue; }
                instr.i_reg[i] = Reg(instr.i_reg[i].0, *self.reg_stack[instr.i_reg[i].0 as usize].1
                                     .last().unwrap() as u16);
            }

            // Rename output register given that the instruction makes use of it
            if instr.o_reg.is_some() && instr.o_reg.unwrap().0 != PReg::Zero {
                // Increase count and push new count onto the stack
                self.reg_stack[instr.o_reg.unwrap().0 as usize].0 += 1;
                let count = self.reg_stack[instr.o_reg.unwrap().0 as usize].0;
                self.reg_stack[instr.o_reg.unwrap().0 as usize].1.push(count);

                let cur_reg = instr.o_reg.unwrap().0;
                instr.o_reg = Some(Reg(cur_reg, *self.reg_stack[cur_reg as usize].1
                                       .last().unwrap() as u16));
            }
        }

        // Go through the successors to fill in phi function parameters
        for succ in self.blocks[block_num].succ.clone() {

            let j = &self.blocks[succ].pred
                .iter()
                .position(|&x| x as usize == block_num)
                .unwrap();

            for instr in &mut self.blocks[succ].phi_funcs {

                let cur_reg = instr.o_reg.unwrap().0;

                if instr.i_reg.len() < j+1 {
                    instr.i_reg.resize(j+1, Reg(PReg::Zero, 0));
                }

                instr.i_reg[*j] = Reg(cur_reg, *self.reg_stack[cur_reg as usize].1
                                       .last().unwrap() as u16);
            }
        }

        // Set before recursive call becase the recursive call's mutable borrow causes issues
        let cur_block_instrs = self.blocks[block_num].instrs(&self.instrs);

        // Retrieve all successors of the current basic block, using the dominator tree instead of,
        // cfg otherwise we will get infinite recursion
        for s in self.idom_tree.clone() {
            if block_num == s.0 as usize {
                self.rename_block(s.1 as usize);
            }
        }

        //// Destroy the accumulated register stack at end of function
        cur_block_instrs.iter()
            .filter_map(|e| e.o_reg)
            .for_each(|e| { self.reg_stack[e.0 as usize].1.pop(); } );
    }

    /// Dump a dot graph for visualization purposes
    pub fn dump_dot(&self) {
        let mut graph = Graph::<_, i32>::new();

        let mut s = String::new();

        for block in &self.blocks {
            println!("block {}: [{}-{}]", block.index, block.start, block.end);
            s.push_str(&format!("\tLabel(0x{:x})\n\n", block.label));
            block.phi_funcs.iter().for_each(|e| { s.push_str(&format!("{}\n", e)); });
            block.instrs(&self.instrs).iter().for_each(|e| { s.push_str(&format!("{}\n", e)); });
            graph.add_node(s.clone());
            s.clear();
        }

        graph.extend_with_edges(&self.edges);

        let mut f = File::create("graph.dot").unwrap();
        let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
        f.write_all(output.as_bytes()).expect("could not write file");
    }
}
