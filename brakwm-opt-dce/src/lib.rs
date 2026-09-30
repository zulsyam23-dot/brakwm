use std::collections::{HashMap, HashSet};
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirFunction, LirInst, LirOpcode, LirOperand, LirProgram, VirtReg};
use brakwm_opt_traits::LirOptimizationPass;

pub struct DeadCodeElimination;

impl LirOptimizationPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dce"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        remove_unused_functions(&mut program);
        for func in &mut program.functions {
            remove_dead_instructions(func);
        }
        Ok(program)
    }
}

fn remove_unused_functions(program: &mut LirProgram) {
    let mut used = HashSet::new();
    used.insert("main".to_string());

    for func in &program.functions {
        for block in &func.blocks {
            for inst in &block.insts {
                if let Some(LirOperand::Label(name)) = inst.operands.first() {
                    if inst.opcode == LirOpcode::Call {
                        used.insert(name.clone());
                    }
                }
            }
        }
    }

    program.functions.retain(|f| used.contains(&f.name));
}

fn remove_dead_instructions(func: &mut LirFunction) {
    let live_out = compute_live_out(func);

    for bi in 0..func.blocks.len() {
        let mut live: HashSet<VirtReg> = live_out.get(&bi).cloned().unwrap_or_default();
        let mut kept = Vec::with_capacity(func.blocks[bi].insts.len());

        for inst in func.blocks[bi].insts.iter().rev() {
            if let Some(d) = inst.dest {
                if !live.contains(&d) && !has_side_effect(inst) {
                    for r in source_regs(inst) {
                        live.insert(r);
                    }
                    continue;
                }
            }

            for r in source_regs(inst) {
                live.insert(r);
            }
            if let Some(d) = inst.dest {
                live.remove(&d);
            }
            kept.push(inst.clone());
        }

        kept.reverse();
        func.blocks[bi].insts = kept;
    }
}

fn compute_live_out(func: &LirFunction) -> HashMap<usize, HashSet<VirtReg>> {
    let succ = compute_successors(func);
    let mut live_out: HashMap<usize, HashSet<VirtReg>> = HashMap::new();
    let n = func.blocks.len();

    let mut changed = true;
    while changed {
        changed = false;
        for bi in 0..n {
            let mut out: HashSet<VirtReg> = HashSet::new();
            if let Some(edges) = succ.get(&bi) {
                for &s in edges {
                    let mut use_set = HashSet::new();
                    let mut live_in: HashSet<VirtReg> = live_out
                        .get(&s)
                        .cloned()
                        .unwrap_or_default();
                    for inst in func.blocks[s].insts.iter() {
                        for r in source_regs(inst) {
                            use_set.insert(r);
                        }
                        if let Some(d) = inst.dest {
                            live_in.remove(&d);
                        }
                    }
                    for r in use_set {
                        live_in.insert(r);
                    }
                    out.extend(live_in);
                }
            }
            if live_out.get(&bi).map_or(true, |v| *v != out) {
                live_out.insert(bi, out);
                changed = true;
            }
        }
    }
    live_out
}

fn compute_successors(func: &LirFunction) -> HashMap<usize, Vec<usize>> {
    let mut succ: HashMap<usize, Vec<usize>> = HashMap::new();
    let block_ids: HashSet<usize> = func.blocks.iter().map(|b| b.id).collect();
    let block_map: HashMap<String, usize> =
        func.blocks.iter().map(|b| (b.name.clone(), b.id)).collect();
    let id_to_idx: HashMap<usize, usize> =
        func.blocks.iter().enumerate().map(|(i, b)| (b.id, i)).collect();

    let resolve = |name: &str| -> Option<usize> {
        let target_id = if let Some(id) = name.strip_prefix("block_").and_then(|s| s.parse::<usize>().ok())
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

fn source_regs(inst: &LirInst) -> Vec<VirtReg> {
    let mut regs = Vec::new();
    for op in &inst.operands {
        if let LirOperand::Reg(r) = op {
            regs.push(*r);
        }
    }
    regs
}

fn has_side_effect(inst: &LirInst) -> bool {
    matches!(
        inst.opcode,
        LirOpcode::Call
            | LirOpcode::Store
            | LirOpcode::Ret
            | LirOpcode::Jmp
            | LirOpcode::Br
            | LirOpcode::SetField
            | LirOpcode::Push
            | LirOpcode::Pop
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(DeadCodeElimination.name(), "dce");
    }
}