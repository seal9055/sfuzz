use crate::{
    irgraph::{Operation, IRGraph, Instruction, Reg},
    emulator::Register as PReg,
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

        self.rename_regs(&dom_tree, &var_list, &var_origin, &phi_func);

        //println!("cfg: {:?}", cfg);
        //println!("leader_set: {:?}", leader_set);
        //println!("dom_tree: {:?}", dom_tree);
        //println!("dom_set: {:?}", dom_set);
        //println!("df_list: {:?}", df_list);
        //println!("\nvar_list: {:?}", var_list);
        //println!("\nvar_origin: {:?}", var_origin);
        //println!("\nvar_list_origin: {:?}", varlist_origin);
        //println!("\nvar_tuple: {:?}", var_tuple);
        //println!("def_sites: {:?}\n", def_sites);
        //println!("var_phi: {:?}\n", var_phi);
        //println!("phi_func: {:?}\n", phi_func);
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
            let pred: Vec<u32> = self.edges.iter().filter(|e| e.1 == n as u32).map(|e| e.0).collect();
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

    fn find_domfrontier(&self, dom_tree: &mut Vec<(isize, isize)>,
            dom_set: &mut Vec<BTreeSet<isize>>) -> Vec<BTreeSet<isize>> {
        let mut v = BTreeSet::new();
        v.insert(0);
        dom_set.insert(0, v);
        dom_tree.insert(0, (-1, 0));

        let mut df_set: Vec<BTreeSet<isize>> = Vec::new();
        dom_set.iter().for_each(|_| df_set.push(BTreeSet::new()));

        for v in &dom_tree.clone() {
            let node = v.1;
            let pred: Vec<u32> = self.edges.iter().filter(|e| e.1 == node as u32).map(|e| e.0).collect();

            for e in pred {
                let mut runner: isize = e as isize;

                while runner != dom_tree[node as usize].0 {
                    let mut new_set = BTreeSet::new();
                    new_set.insert(node);
                    df_set[runner as usize] = new_set.clone();
                    runner = dom_tree[runner as usize].0;
                }
            }
        }
        df_set
    }

    fn find_var_origin(&self)
        -> (Vec<Reg>, Vec<(Reg, usize)>, Vec<Vec<Reg>>, Vec<(Reg, usize)>) {

        let mut var_origin = Vec::new();

        //// Extract all register definitions from the function
        let mut i = 0;
        for block in &self.blocks {
            for instr in block {
                let out_reg = match instr.o_reg {
                    Some(reg) => reg,
                    _ => Reg(PReg::Zero, 0),
                };
                if out_reg.0 != PReg::Zero {
                    var_origin.push((out_reg, i));
                }
                i += 1;
            }
        }

        println!("var_origin: {:?}", var_origin);
        let mut leader_set_index: Vec<usize> = self.leader_set.iter().map(|e| e.1).collect();
        leader_set_index.push(self.num_instrs);

        let mut varnode_origin: Vec<usize> = Vec::new();
        i = 0;

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

    fn insert_phi_func(&self, df_list: &Vec<BTreeSet<isize>>, varlist_origin: &Vec<Vec<Reg>>,
                       var_tuple: &Vec<(Reg, usize)>)
        -> (Vec<Vec<usize>>, Vec<BTreeSet<usize>>, Vec<BTreeSet<(Reg, isize)>>) {

        let mut def_sites: Vec<Vec<usize>>      = vec![Vec::new(); 34];
        let mut var_phi:   Vec<BTreeSet<usize>> = Vec::new();
        let mut phi_func:  Vec<BTreeSet<(Reg, isize)>> = Vec::new();

        for v in var_tuple {
            def_sites[v.0.0 as usize].push(v.1);
            var_phi.push(BTreeSet::new());
        }

        for _ in 0..df_list.len() {
            phi_func.push(BTreeSet::new());
        }

        let mut count = 0;
        for (i, var) in def_sites.iter().enumerate() {
            if var.len() == 0 { continue; }
            count += 1;
            let mut temp_list = def_sites[i].clone();

            while let Some(n) = temp_list.pop() {
                for y in &df_list[n] {
                    if !var_phi[count].contains(&(*y as usize)) {
                        let len = self.edges.iter().filter(|x| x.1 as isize == *y).count() as isize;
                        phi_func[*y as usize].insert((Reg(PReg::from(i as u32), 0), len));
                        var_phi[count].insert(*y as usize);
                        if varlist_origin[n].iter().find(|&&x| x == Reg(PReg::from(*y as u32), 0))
                            .is_none() {
                            temp_list.push(*y as usize);
                        }
                    }
                }
            }
        }
        (def_sites, var_phi, phi_func)
    }

    fn rename_regs(&mut self, dom_tree: &Vec<(isize, isize)>, var_list: &Vec<Reg>,
                var_origin: &Vec<(Reg, usize)>, phi_func: &Vec<BTreeSet<(Reg, isize)>>) {

        let mut var_dict: Vec<(usize, Vec<usize>)> = vec![(0, Vec::new()); 32];

        for var in var_list {
            var_dict[var.0 as usize] = (0, vec![0; 1]);
        }

        let mut phi_func_mod: Vec<Vec<Vec<(Reg, isize)>>>  = Vec::new();
        let mut phi_func_temp: Vec<Vec<Vec<Reg>>> = Vec::new();
        let mut blocks: Vec<((usize, usize), usize)> = Vec::new();

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

        let mut rename_block = |block: ((usize, usize), usize)| {
            let block_line_nums = block.0;
            //let block_num       = block.1;
            let block_num       = 2; // TODO Revert
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

                let mut new_instr = each_line.1;
                //new_instr.rd
                //TODO

                println!("new_instr: {:?}", new_instr);
            }

        };

        println!("blocks: {:?}", blocks);
        rename_block(blocks[0]);
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
