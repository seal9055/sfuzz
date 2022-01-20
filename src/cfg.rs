use crate::{
    irgraph::{Operation, IRGraph, Instruction},
};

use std::fs::File;
use std::io::Write;

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

        while let Some(&instr) = iterator.next() {
            match instr.op {
                Operation::Label(v) => { /* Handles labels */
                    block.clear();
                    block.push(instr);
                    index += 1;
                    map.insert(v, index);
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
        }

        if !block.is_empty() { cfg.blocks.push(block.clone()); }

        for edge in edges {
            let v = *(map.get(&(edge.1 as usize)).unwrap()) as u32;
            cfg.edges.push((edge.0, v));
        }

        cfg
    }

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
