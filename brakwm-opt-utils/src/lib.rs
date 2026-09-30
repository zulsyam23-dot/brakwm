use std::collections::{HashMap, HashSet};
use brakwm_ir_lir::lir::{LirFunction, LirOpcode, LirOperand};

/// Successors keyed by block VEC INDEX.
pub fn cfg_successors(func: &LirFunction) -> HashMap<usize, Vec<usize>> {
    let mut succ: HashMap<usize, Vec<usize>> = HashMap::new();
    let block_ids: HashSet<usize> = func.blocks.iter().map(|b| b.id).collect();
    let block_map: HashMap<String, usize> =
        func.blocks.iter().map(|b| (b.name.clone(), b.id)).collect();
    let id_to_idx: HashMap<usize, usize> =
        func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();

    let resolve = |name: &str| -> Option<usize> {
        let target_id = if let Some(id) = name
            .strip_prefix("block_")
            .and_then(|s| s.parse::<usize>().ok())
        {
            if block_ids.contains(&id) {
                Some(id)
            } else {
                None
            }
        } else {
            block_map.get(name).copied()
        }?;
        id_to_idx.get(&target_id).copied()
    };

    for (bi, block) in func.blocks.iter().enumerate() {
        let edges = succ.entry(bi).or_default();
        for inst in &block.insts {
            match inst.opcode {
                LirOpcode::Jmp => {
                    if let Some(LirOperand::Label(name)) = inst.operands.first() {
                        if let Some(t) = resolve(name) {
                            edges.push(t);
                        }
                    }
                }
                LirOpcode::Br => {
                    for op in inst.operands.iter().skip(1) {
                        if let LirOperand::Label(name) = op {
                            if let Some(t) = resolve(name) {
                                edges.push(t);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
    }
    succ
}

/// PredPCs keyed by block VEC INDEX.
pub fn cfg_predecessors(succ: &HashMap<usize, Vec<usize>>) -> HashMap<usize, Vec<usize>> {
    let mut pred: HashMap<usize, Vec<usize>> = HashMap::new();
    for (&b, edges) in succ {
        for e in edges {
            pred.entry(*e).or_default().push(b);
        }
    }
    pred
}

/// Natural loop headers, keyed by VEC INDEX. A block is a loop header if it
/// has an incoming edge from a block it dominates (a back edge).
pub fn loop_headers(
    func: &LirFunction,
    succ: &HashMap<usize, Vec<usize>>,
) -> HashSet<usize> {
    let n = func.blocks.len();
    if n == 0 {
        return HashSet::new();
    }
    let dom = dominators(func, succ);
    let mut headers = HashSet::new();
    for (&b, edges) in succ {
        for e in edges {
            // b -> e is a back edge if b dominates e.
            if dom.get(e).map(|s| s.contains(&b)).unwrap_or(false) {
                headers.insert(*e);
            }
        }
    }
    headers
}

/// Dominators keyed by VEC INDEX (including self).
pub fn dominators(
    func: &LirFunction,
    succ: &HashMap<usize, Vec<usize>>,
) -> HashMap<usize, HashSet<usize>> {
    let n = func.blocks.len();
    let all: HashSet<usize> = (0..n).collect();
    let pred = cfg_predecessors(succ);

    let mut dom: HashMap<usize, HashSet<usize>> = HashMap::new();
    dom.insert(0, HashSet::from([0]));
    for i in 1..n {
        dom.insert(i, all.clone());
    }

    let mut changed = true;
    while changed {
        changed = false;
        for i in 0..n {
            let new: HashSet<usize> = match pred.get(&i) {
                Some(ps) if !ps.is_empty() => {
                    let mut inter = dom.get(&ps[0]).cloned().unwrap_or_default();
                    for p in &ps[1..] {
                        let d = dom.get(p).cloned().unwrap_or_default();
                        inter = inter.intersection(&d).copied().collect();
                    }
                    inter.insert(i);
                    inter
                }
                _ => HashSet::from([i]),
            };
            if dom.get(&i).map(|s| *s != new).unwrap_or(true) {
                dom.insert(i, new);
                changed = true;
            }
        }
    }
    dom
}

#[cfg(test)]
mod tests {
    use super::*;
    use brakwm_ir_lir::lir::{LirBlock, LirInst, LirType};

    fn simple_loop_func() -> LirFunction {
        // entry -> {body, exit}; body -> body (self loop)
        let mut blocks = Vec::new();
        blocks.push(LirBlock {
            id: 0,
            name: "entry".to_string(),
            insts: Vec::new(),
            span: Default::default(),
        });
        blocks.push(LirBlock {
            id: 1,
            name: "body".to_string(),
            insts: vec![LirInst::new(LirOpcode::Jmp)
                .with_op(LirOperand::Label("block_1".to_string()))],
            span: Default::default(),
        });
        blocks.push(LirBlock {
            id: 2,
            name: "exit".to_string(),
            insts: Vec::new(),
            span: Default::default(),
        });
        LirFunction {
            name: "f".to_string(),
            params: vec![],
            ret_ty: LirType::I32,
            reg_types: vec![],
            blocks,
            reg_count: 0,
            span: Default::default(),
        }
    }

    #[test]
    fn detects_loop_header() {
        let f = simple_loop_func();
        let succ = cfg_successors(&f);
        let headers = loop_headers(&f, &succ);
        assert!(headers.contains(&1), "body should be a loop header");
    }
}