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
/// (start_index, end_index), id)
#[derive(Debug, Default, Copy, Clone)]
pub struct Block(pub (usize, usize), pub usize);

impl Block {
    /// The instructions used in a block
    pub fn instrs(&self, instrs: &[Instruction]) -> Vec<Instruction> {
        instrs[self.0.0..self.0.1].to_vec()
    }

    /// The set of registers used in a given block
    pub fn regs_uses(&self, instrs: &[Instruction]) -> Vec<Reg> {
        self.instrs(instrs)
            .iter()
            .flat_map(|e| &e.i_reg)
            .copied()
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

    /// Calculated phi nodes
    pub phi_func: Vec<BTreeSet<(Reg, isize)>>,

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
        let mut i = 0;

        // Determine labels locations
        while let Some(instr) = &iterator.next() {
            match instr.op {
                Operation::Label(v) => { /* Handles labels */
                    index += 1;
                    map.insert(v, index);
                    ssa_builder.leader_set.push(((*instr).clone(), i));
                },
                Operation::Branch(x, y) => { /* End basic block with a branch to 2 other blocks */
                    edges.push((index as u32, y as u32));
                    edges.push((index as u32, x as u32));
                }
                Operation::Jmp(x) => { /* End basic block with a non-returning jmp */
                    edges.push((index as u32, x as u32));
                },
                Operation::Ret => {}, /* End basic block with a return */
                _ => {
                    // Insert an edge if next instruction is a label
                    if iterator.peek().is_some() {
                        if let Operation::Label(x) = iterator.peek().unwrap().op {
                            edges.push((index as u32, x as u32));
                        }
                    }
                }
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
            ssa_builder.blocks.push(Block((v.1, ssa_builder.leader_set[i+1].1), i));
            if i == ssa_builder.leader_set.len()-2 { break; }
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
        self.phi_func = self.calculate_phi_funcs(&varlist_origin, &var_tuple);
        self.insert_phi_funcs();
        self.rename(&var_list);
    }

    /*
        Algorithm:
            For every block in the graph call the dom closure. This function loops through all
            previous blocks that lead to the current block and add them to the idom and dom tree's
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
            let pred: Vec<u32> = self.edges.iter().filter(|e| e.1 == n as u32)
                .map(|e| e.0).collect();
            let mut dom_check: Vec<BTreeSet<isize>> = Vec::new();
            pred.iter().for_each(|e| { dom_check.push(dom_temp[*e as usize].clone()); });

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

        let mut dominance_frontier: Vec<BTreeSet<isize>> = Vec::new();

        // Create an index in the set for every node in the graph
        dom_tree.iter().for_each(|_| dominance_frontier.push(BTreeSet::new()));

        for v in &self.idom_tree.clone() {
            let join_point = v.1;

            // Collect all predecessor nodes for the current join point from the cfg
            let predecessors: Vec<u32> = self.edges.iter().filter(|e| e.1 == join_point as u32)
                .map(|e| e.0).collect();

            // Loop through all predecessors of the join point. If a predecessor is not an idom,
            // insert it into the dominance frontier set
            for p in predecessors {
                let mut runner: isize = p as isize;

                while runner != self.idom_tree[join_point as usize].0 {
                    dominance_frontier[runner as usize] = btreeset!{join_point as isize};
                    runner = self.idom_tree[runner as usize].0;
                }
            }
        }
        dominance_frontier
    }

    /*
       Returns a couple of structures describing def/use relationships between registers and their
       corresponding blocks. These are useful to simplify the algorithms in future functions.
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
    fn calculate_phi_funcs(&self, varlist_origin: &[Vec<Reg>], var_tuple: &[(Reg, usize)])
        -> Vec<BTreeSet<(Reg, isize)>> {

        let mut def_sites: Vec<Vec<usize>>      = vec![Vec::new(); NUMREGS];
        let mut var_phi:   Vec<BTreeSet<usize>> = Vec::new();
        let mut phi_func = vec![BTreeSet::new(); self.dominance_frontier.len()];

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
                        let len = self.edges.iter().filter(|e| e.1 as isize == *x).count() as isize;
                        phi_func[*x as usize].insert((Reg(PReg::from(i as u32), 0), len));
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
        phi_func
    }

    /// Actually insert the phi functions previously calculated into the instruction array
    fn insert_phi_funcs(&mut self) {
        for (i, phi_function) in self.phi_func.iter().enumerate() {
            let start_index = self.blocks[i].0.0+1;
            for input in phi_function {
                let a = Instruction  {
                    op: Operation::Phi,
                    i_reg: Vec::new(),
                    o_reg: Some(input.0),
                    flags: 0,
                    pc: None,
                };
                self.instrs.insert(start_index, a);

                // Since we are inserting instructions, the blocks vector needs to be updated to
                // reflect this
                self.blocks[i].0.1 += 1;
                for j in i+1..self.blocks.len() {
                    self.blocks[j].0.0 += 1;
                    self.blocks[j].0.1 += 1;
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
        // List of basic blocks in this program ((block_start, block_end), block_number)
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
        let basic_block = self.blocks[block_num];

        // Rename any existing phi functions at the start of the function
        for i in basic_block.0.0+1..basic_block.0.1 {
            let mut instr = &mut self.instrs[i];
            if instr.op != Operation::Phi { break; }

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

        // Rename inputs and outputs
        for i in basic_block.0.0..basic_block.0.1 {
            let mut instr = &mut self.instrs[i];

            if instr.op == Operation::Phi { continue; }

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

        // Retrieve all successors of the current basic block
        let successors: Vec<u32> = self.edges.iter()
            .filter(|v| v.0 == block_num as u32).map(|e| e.1).collect();

        // Go through the successors to fill in phi function parameters
        for s in &successors {
            let succ_block = self.blocks[*s as usize];

            let pred: Vec<u32> = self.edges.iter()
                .filter(|v| v.1 == *s as u32).map(|e| e.0).collect();

            let j = pred.iter().position(|&x| x as usize == block_num).unwrap();

            for i in succ_block.0.0+1..succ_block.0.1 {
                let instr = &mut self.instrs[i];
                if instr.op != Operation::Phi { break; }

                let cur_reg = instr.o_reg.unwrap().0;

                if instr.i_reg.len() < j+1 {
                    instr.i_reg.resize(j+1, Reg(PReg::Zero, 0));
                }

                instr.i_reg[j] = Reg(cur_reg, *self.reg_stack[cur_reg as usize].1
                                       .last().unwrap() as u16);
            }
        }

        // Retrieve all successors of the current basic block, using the dominator tree instead of,
        // cfg otherwise we will get infinite recursion
        for s in self.idom_tree.clone() {
            if block_num == s.0 as usize {
                self.rename_block(s.1 as usize);
            }
        }

        // Destroy the accumulated register stack at end of function
        for i in basic_block.0.0..basic_block.0.1 {
            let instr = &self.instrs[i];

            if instr.o_reg.is_some() && instr.o_reg.unwrap().0 != PReg::Zero {
                self.reg_stack[instr.o_reg.unwrap().0 as usize].1.pop();
            }
        }
    }

    /// Dump a dot graph for visualization purposes
    pub fn dump_dot(&self) {
        let mut graph = Graph::<_, i32>::new();

        let mut s = String::new();
        let mut count = 0;
        for (i, instr) in self.instrs.iter().enumerate() {
            s.push_str(&format!("{}\n", instr));

            if i == self.blocks[count].0.1-1 { /* End of block reached */
                s.push_str("\n ");
                graph.add_node(s.clone());
                count += 1;
                s.clear();
                s.push('\n');
            }
        }

        graph.extend_with_edges(&self.edges);

        let mut f = File::create("graph.dot").unwrap();
        let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
        f.write_all(output.as_bytes()).expect("could not write file");
    }
}
