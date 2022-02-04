use crate::{
    irgraph::{Operation, IRGraph, Instruction, Reg},
    emulator::Register as PReg,
    maplit,
};

use std::fs::File;
use std::io::Write;
use std::collections::{BTreeMap, BTreeSet};

use petgraph::Graph;
use petgraph::dot::{Dot, Config};
use rustc_hash::FxHashMap;


/// Controlflow graph used in the ir
#[derive(Debug, Default)]
pub struct CFG {
    /// A a vector of basic blocks in the cfg. These hold all instructions
    pub blocks: Vec<Vec<Instruction>>,

    /// Map the index of a block in the blocks array to the index of the block its jumping to
    pub edges: Vec<(u32, u32)>,

    /// Number of instructions in the current cfg
    pub num_instrs: usize,

    /// List of all instructions in a given graph
    pub instrs: Vec<Instruction>,

    /// Maps all labels to their instruction index
    pub leader_set: Vec<(Instruction, usize)>,
}

impl CFG {
    /// Loop through the instructions of a given function and generate the control flow graph for it
    pub fn new(irgraph: &IRGraph) -> CFG {
        let mut cfg   = CFG::default();
        let mut block = Vec::new();
        let mut index: isize = -1;
        let mut iterator = irgraph.instrs.iter().peekable();
        let mut map: FxHashMap<usize, isize> = FxHashMap::default();
        let mut edges = Vec::new();
        let mut i = 0;

        while let Some(&instr) = iterator.next() {
            match instr.op {
                Operation::Label(v) => { /* Handles labels */
                    block.clear();
                    block.push(instr);
                    index += 1;
                    map.insert(v, index);
                    cfg.leader_set.push((instr, i));
                },
                Operation::Branch(x, y) => { /* End basic block with a branch to 2 other blocks */
                    block.push(instr);
                    cfg.blocks.push(block.clone());
                    block.clear();
                    edges.push((index as u32, y as u32));
                    edges.push((index as u32, x as u32));
                }
                Operation::Jmp(x) => { /* End basic block with a non-returning jmp */
                    block.push(instr);
                    cfg.blocks.push(block.clone());
                    block.clear();
                    edges.push((index as u32, x as u32));
                },
                Operation::Ret => { /* End basic block with a non-returning jmp */
                    block.push(instr);
                    cfg.blocks.push(block.clone());
                    block.clear();
                },
                _ => { /* Matches all other instruction */
                    block.push(instr);

                    // Insert an edge if next instruction is a label
                    if iterator.peek().is_some() {
                        match iterator.peek().unwrap().op {
                            Operation::Label(x) => {
                                edges.push((index as u32, x as u32));
                                cfg.blocks.push(block.clone());
                                block.clear();
                            },
                            _ => {},
                        }
                    }
                }
            }
            i+=1;
        }

        if !block.is_empty() { cfg.blocks.push(block.clone()); }

        for edge in edges {
            let v = *(map.get(&(edge.1 as usize)).unwrap()) as u32;
            cfg.edges.push((edge.0, v));
        }

        cfg.num_instrs = irgraph.instrs.len();

        cfg
    }

    /// Convert CFG graph to single static assignment form
    pub fn convert_to_ssa(&mut self) {

        let (mut dom_tree, mut dom_set) = self.generate_domtree();
        let df_list = self.find_domfrontier(&mut dom_tree, &mut dom_set);
        let (var_list, var_origin, varlist_origin, var_tuple) = self.find_var_origin();

        let (def_sites, var_phi, phi_func) =
            self.insert_phi_func(&df_list, &varlist_origin, &var_tuple);

        //println!("cfg: {:?}\n\n", self.edges);
        //println!("leader_set: {:?}", self.leader_set);
        //println!("dom_tree: {:?}", dom_tree);
        //println!("dom_set: {:?}", dom_set);
        //println!("df_list: {:?}", df_list);
        //println!("\nvar_list: {:?}", var_list);
        //println!("\nvar_origin: {:?}", var_origin);
        //println!("\nvar_list_origin: {:?}", varlist_origin);
        //println!("\nvar_tuple: {:?}", var_tuple);
        //println!("def_sites: {:?}\n", def_sites);
        println!("var_phi: {:?}\n", var_phi);
        println!("phi_func: {:?}\n", phi_func);

        self.rename_regs(&dom_tree, &var_list, &var_origin, &phi_func);
    }

    /// Generate dominator tree
    fn generate_domtree(&mut self) -> (Vec<(isize, isize)>, Vec<BTreeSet<isize>>) {
        let initial: isize = self.edges[0].0 as isize;
        let mut dom_temp: Vec<BTreeSet<isize>> = Vec::new();

        let num_leaders = self.leader_set.len();

        for i in 0..num_leaders {
            let mut v: BTreeSet<isize> = BTreeSet::new();
            v.insert(initial);
            v.insert(i as isize);
            dom_temp.push(v);
        }

        let mut dom = move |n: usize| {
            let mut dom_set = dom_temp[n].clone();
            let pred: Vec<u32> = self.edges.iter().filter(|e| e.1 == n as u32)
                .map(|e| e.0).collect();
            let mut dom_check: Vec<BTreeSet<isize>> = Vec::new();
            pred.iter().for_each(|e| { dom_check.push(dom_temp.iter().nth(*e as usize)
                                                      .unwrap().clone()); });

            let dom_inter = dom_check.iter().nth(0).unwrap();
            let dom_inter = dom_check.iter().fold(BTreeSet::new(), |_, e| {
                e.intersection(&dom_inter).collect()
            });

            dom_inter.iter().for_each(|e| { dom_set.insert(**e); });
            dom_set.iter().for_each(|e| { dom_temp[n].insert(*e); });
            dom_set
        };

        let mut dom_tree: Vec<(isize, isize)> = Vec::new();
        let mut dom_set: Vec<BTreeSet<isize>> = Vec::new();

        for i in 1..num_leaders {
            let mut dom_tempset: BTreeSet<isize> = dom(i);

            dom_set.push(dom_tempset.clone());
            dom_tempset.remove(&(i as isize));
            let max_val = dom_tempset.into_iter().max().unwrap();
            dom_tree.push((max_val as isize, i as isize));
        }

        (dom_tree, dom_set)
    }

    /*
        Algorithm:
            Locate all join points j
            For each predecessor p of j, walk up the dominator tree from p until a node is found
            that dominates j. All nodes in this traversal from p to the node that dominates j, but
            not including j belong to the dominance frontier.
    */
    /// Find Dominance Frontiers
    fn find_domfrontier(&self, dom_tree: &mut Vec<(isize, isize)>,
            dom_set: &mut Vec<BTreeSet<isize>>) -> Vec<BTreeSet<usize>> {

        // Add an extra node at the beginning of the graph that dominates everything.
        // This makes the implementation a little simpler
        dom_set.insert(0, btreeset!{0});
        dom_tree.insert(0, (-1, 0));

        let mut df_set: Vec<BTreeSet<usize>> = Vec::new();

        // Create an index in the set for every node in the graph
        dom_set.iter().for_each(|_| df_set.push(BTreeSet::new()));

        for v in &dom_tree.clone() {
            let join_point = v.1;

            // Collect all predecessor nodes for the current join point from the cfg
            let predecessors: Vec<u32> = self.edges.iter().filter(|e| e.1 == join_point as u32)
                .map(|e| e.0).collect();

            // Loop through all predecessors of the join point. If a predecessor is not an idom,
            // insert it into the dominance frontier set
            for p in predecessors {
                let mut runner: isize = p as isize;

                while runner != dom_tree[join_point as usize].0 {
                    df_set[runner as usize] = btreeset!{join_point as usize};
                    runner = dom_tree[runner as usize].0;
                }
            }
        }
        df_set
    }

    /*
       Returns a couple of structures describing def/use relationships between registers and their
       corresponding blocks. These are useful to simplify the algorithms in future functions.
    */
    ///
    fn find_var_origin(&self)
        -> (Vec<Reg>, Vec<(Reg, usize)>, Vec<Vec<Reg>>, Vec<(Reg, usize)>) {

        let mut var_origin = Vec::new();

        //// Extract all register definitions from the function
        for (i, instr) in self.instrs.iter().enumerate() {
            if instr.o_reg.is_some() && instr.o_reg.unwrap().0 != PReg::Zero {
                var_origin.push((instr.o_reg.unwrap(), i));
            }
        }

        let mut leader_set_index: Vec<usize> = self.leader_set.iter().map(|e| e.1).collect();
        leader_set_index.push(self.num_instrs);

        let mut varnode_origin: Vec<usize> = Vec::new();
        let mut i = 0;

        for x in &var_origin {
            let instr_index = x.1;
            if instr_index >= leader_set_index[i+1] { i += 1; }
            varnode_origin.push(i);
        }

        let mut varlist_temp: Vec<(Reg, usize)> = Vec::new();
        for i in 0..var_origin.len() {
            varlist_temp.push((var_origin[i].0, varnode_origin[i]));
        }

        let mut varlist_origin: Vec<Vec<Reg>> = Vec::new();
        for v in &varlist_temp {
            if varlist_origin.len() <= v.1 {
                let tmp: Vec<Reg> = Vec::new();
                varlist_origin.push(tmp);
            }
            varlist_origin[v.1].push(v.0);
        }

        let var_list = var_origin.iter().map(|v| v.0).collect();

        (var_list, var_origin, varlist_origin, varlist_temp)
    }

    /*
        Algorithm:
            Whenever a register x is defined in a block b, a phi function needs to be inserted at
            the start of every dominance frontier of b. Since every phi function insertion may lead
            to more phi functions being inserted, we need to loop through all potential register
            definitions after every insertion.
    */
    ///
    fn insert_phi_func(&self, domination_frontier: &Vec<BTreeSet<usize>>,
                       varlist_origin: &Vec<Vec<Reg>>, var_tuple: &Vec<(Reg, usize)>)
        -> (Vec<Vec<usize>>, Vec<BTreeSet<usize>>, Vec<BTreeSet<(Reg, isize)>>) {

        let mut def_sites: Vec<Vec<usize>>      = vec![Vec::new(); 34];
        let mut var_phi:   Vec<BTreeSet<usize>> = Vec::new();
        let mut phi_func = vec![BTreeSet::new(); domination_frontier.len()];

        // Vector of all registers, each index contains a vector that lists all blocks in which its
        // register was declared
        for v in var_tuple {
            def_sites[v.0.0 as usize].push(v.1);
            var_phi.push(BTreeSet::new());
        }

        let mut count = 0;
        for (i, var) in def_sites.iter().enumerate() {
            if var.len() == 0 { continue; }
            count += 1;

            // worklist of blocks
            let mut worklist = def_sites[i].clone();

            while let Some(block) = worklist.pop() {
                for x in &domination_frontier[block] {
                    // If the block has no phi functions for x
                    if !var_phi[count].contains(&(*x as usize)) {

                        // Insert phi functions
                        let len = self.edges.iter().filter(|e| e.1 as usize == *x).count() as isize;
                        phi_func[*x as usize].insert((Reg(PReg::from(i as u32), 0), len));
                        var_phi[count].insert(*x as usize);

                        // if x is not in varlist_origin, update the worklist
                        if varlist_origin[block].iter().
                            find(|&&e| e == Reg(PReg::from(*x as u32), 0)).is_none() {
                            worklist.push(*x as usize);
                        }
                    }
                }
            }
        }
        (def_sites, var_phi, phi_func)
    }

    fn rename_regs(&mut self, dom_tree: &Vec<(isize, isize)>, var_list: &Vec<Reg>,
                var_origin: &Vec<(Reg, usize)>, phi_func: &Vec<BTreeSet<(Reg, isize)>>) {

        // Count/Stack used for variable renaming
        let mut var_dict: Vec<(usize, Vec<usize>)> = vec![(0, Vec::new()); 34];

        let mut phi_func_mod: Vec<Vec<Vec<(Reg, isize)>>> = Vec::new();
        let mut phi_func_temp: Vec<Vec<Vec<Reg>>>         = Vec::new();
        let mut blocks: Vec<((usize, usize), usize)>      = Vec::new();

        // Initialize the current positions for all used registers in this function to 0
        for var in var_list {
            var_dict[var.0 as usize] = (0, vec![0; 1]);
        }

        for (i, var) in phi_func.iter().enumerate() {
            if phi_func_temp.len() <= i {
                phi_func_temp.push(Vec::new());
                phi_func_mod.push(Vec::new());
            }
            for t in &phi_func[i] {
                phi_func_temp[i].push(vec![t.0; t.1 as usize]);
            }
        }
        self.leader_set.push((Instruction::default(), self.num_instrs));

        for (i, v) in self.leader_set.iter().enumerate() {
            blocks.push(((v.1, self.leader_set[i+1].1), i));
            if i == self.leader_set.len()-2 { break; }
        }


        //println!("blocks: {:?}", blocks);
        self.rename_block(&blocks, 0, phi_func, &mut var_dict, &mut phi_func_mod, var_origin,
                          &mut phi_func_temp, dom_tree);
    }

    fn rename_block(&mut self, blocks: &Vec<((usize, usize), usize)>,
                    index: usize,
                    phi_func: &Vec<BTreeSet<(Reg, isize)>>,
                    var_dict: &mut Vec<(usize, Vec<usize>)>,
                    phi_func_mod: &mut Vec<Vec<Vec<(Reg, isize)>>>,
                    var_origin: &Vec<(Reg, usize)>,
                    phi_func_temp: &mut Vec<Vec<Vec<Reg>>>,
                    dom_tree: &Vec<(isize, isize)>) {

        let block = blocks[index];
        let block_line_nums = block.0;
        let block_num       = block.1;
        let mut block_lines = Vec::new();

        for i in block_line_nums.0..block_line_nums.1 {
            block_lines.push((i, self.instrs[i]));
        }

        for each in &phi_func[block_num] {
            var_dict[each.0.0 as usize].0 += 1;
            let x = var_dict[each.0.0 as usize].0;
            var_dict[each.0.0 as usize].1.push(x);
            let add = (each.0, each.1);
            //println!("add: {:?}", add);
            let mut n_each = vec![(each.0, 1); 1];
            n_each.push(add);
            //println!("n_each: {:?}", n_each);

            phi_func_mod[block_num].push(n_each);
            //println!("phi_func_mod: {:?}", phi_func_mod);
        }

        for (i, each_line) in block_lines.iter().enumerate() {
            let mut def_var: Reg = Reg(PReg::Zero, 0);
            for var in var_origin {
                if var.1 == each_line.0 {
                    def_var = var.0;
                }
            }
            //println!("def_var: {:?}", def_var);

            var_dict[def_var.0 as usize].0 += 1;
            let x = var_dict[def_var.0 as usize].0;
            var_dict[def_var.0 as usize].1.push(x);

            //println!("var_dict: {:?}", var_dict);

            //println!("old_var: {:?}", each_line);
            if each_line.1.o_reg.is_some() {
                each_line.1.o_reg.unwrap().1 = var_dict[def_var.0 as usize].1[0] as u16;
            }

            // TODO replace instruction with new instruction

            let mut var_loc = false;
            let mut var_use: Vec<Reg> = Vec::new();

            // Check for input registers
            if each_line.1.i_reg.0.is_some() {
                var_use.push(each_line.1.i_reg.0.unwrap());
            }
            if each_line.1.i_reg.1.is_some() {
                var_use.push(each_line.1.i_reg.1.unwrap());
            }

            for (i, each_use) in var_use.iter().enumerate() {
                match i {
                    0 => { each_line.1.i_reg.0.unwrap().1 =
                        var_dict[each_use.0 as usize].1[0] as u16; },
                    1 => { each_line.1.i_reg.1.unwrap().1 =
                        var_dict[def_var.0 as usize].1[0] as u16; },
                    _ => {},
                }
                // TODO this does not work
            }
            // TODO replace instruction with new instruction
        }

        let list: Vec<u32> = self.edges.iter()
            .filter(|v| v.0 == block_num as u32).map(|e| e.1).collect();

        for succ in list {
            let pred: Vec<u32> = self.edges.iter()
                .filter(|v| v.1 == succ as u32).map(|e| e.1).collect();

            let j = pred[block_num] as usize;

            for func in &mut phi_func_temp[succ as usize] {
                func[j].1 = var_dict[func[j].0 as usize].1[1] as u16;
            }
        }
        let child: Vec<isize> = dom_tree.iter()
            .filter(|v| v.0 == block_num as isize).map(|e| e.1).collect();

        for each_child in child {
            self.rename_block(blocks, each_child as usize, phi_func, var_dict, phi_func_mod,
                              var_origin, phi_func_temp, dom_tree);
        }

        for each_def in &phi_func[block_num] {
            var_dict[each_def.0.0 as usize].1.pop();
        }
    }

    /// Dump a dot graph for visualization purposes
    pub fn dump_dot(&self) {
        let mut graph = Graph::<_, i32>::new();

        for block in &self.blocks {
            let mut s = String::new();
            let mut first = true;
            for instr in block {
                if first {
                    first = false;
                    s.push_str(&format!("{}", instr));
                } else {
                    s.push_str(&format!("\n{}", instr));
                }
            }
            s.push_str("\n ");
            graph.add_node(s);
        }

        graph.extend_with_edges(&self.edges);

        let mut f = File::create("graph.dot").unwrap();
        let output = format!("{}", Dot::with_config(&graph, &[Config::EdgeNoLabel]));
        f.write_all(output.as_bytes()).expect("could not write file");
    }
}
