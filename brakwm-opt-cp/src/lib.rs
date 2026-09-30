use std::collections::HashMap;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram, VirtReg};
use brakwm_opt_traits::LirOptimizationPass;
use brakwm_opt_utils::cfg_successors;

pub struct CopyPropagation;

impl LirOptimizationPass for CopyPropagation {
    fn name(&self) -> &'static str {
        "cp"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        for func in &mut program.functions {
            propagate(func);
        }
        Ok(program)
    }
}

fn propagate(func: &mut brakwm_ir_lir::lir::LirFunction) {
    let succ = cfg_successors(func);
    // Forward dataflow to find reaching copies reg -> const / reg.
    let mut copy_out: HashMap<usize, HashMap<VirtReg, LirOperand>> = HashMap::new();
    let mut changed = true;

    while changed {
        changed = false;
        for bi in 0..func.blocks.len() {
            let mut preds: Vec<usize> = Vec::new();
            for (&b, edges) in succ.iter() {
                if edges.contains(&bi) {
                    preds.push(b);
                }
            }

            // Walk the block in order: copies generate, other defs kill.
            let mut gen: HashMap<VirtReg, LirOperand> = HashMap::new();
            let mut kill: Vec<VirtReg> = Vec::new();
            for inst in &func.blocks[bi].insts {
                if let Some(d) = inst.dest {
                    if inst.opcode == LirOpcode::Mov && copies_reg(inst) {
                        if let Some(LirOperand::Reg(r)) = inst.operands.first() {
                            kill.retain(|k| *k != d);
                            gen.insert(d, LirOperand::Reg(*r));
                            continue;
                        }
                    }
                    if !gen.remove(&d).is_some() {
                        kill.push(d);
                    }
                }
            }

            // Copies available on entry: intersection of all predecessors.
            let mut in_copies: HashMap<VirtReg, LirOperand> = HashMap::new();
            if let Some(&first) = preds.first() {
                if let Some(c) = copy_out.get(&first) {
                    in_copies = c.clone();
                }
            }
            for &p in preds.iter().skip(1) {
                if let Some(c) = copy_out.get(&p) {
                    in_copies.retain(|k, v| c.get(k) == Some(v));
                }
            }

            for k in &kill {
                in_copies.remove(k);
            }
            in_copies.extend(gen);

            if copy_out.get(&bi).map_or(true, |c| *c != in_copies) {
                copy_out.insert(bi, in_copies);
                changed = true;
            }
        }
    }

    // Apply: replace Reg(x) with its copy source when available.
    for bi in 0..func.blocks.len() {
        let copies = copy_out.get(&bi).cloned().unwrap_or_default();
        let block = &func.blocks[bi];
        let mut new_insts = Vec::with_capacity(block.insts.len());
        let mut live = copies.clone();
        for inst in &block.insts {
            let mut new_inst = inst.clone();
            for op in new_inst.operands.iter_mut() {
                if let LirOperand::Reg(r) = op {
                    if let Some(src) = live.get(r) {
                        if let LirOperand::Reg(sr) = src {
                            if *sr != *r && !defines(new_inst.opcode) {
                                *op = LirOperand::Reg(*sr);
                            }
                        }
                    }
                }
            }
            if let Some(d) = new_inst.dest {
                live.remove(&d);
            }
            new_insts.push(new_inst);
        }
        func.blocks[bi].insts = new_insts;
    }
}

fn copies_reg(inst: &LirInst) -> bool {
    inst.opcode == LirOpcode::Mov
        && inst.operands.len() == 1
        && matches!(inst.operands.first(), Some(LirOperand::Reg(_)))
}

fn defines(op: LirOpcode) -> bool {
    matches!(
        op,
        LirOpcode::Mov
            | LirOpcode::Add
            | LirOpcode::Sub
            | LirOpcode::Mul
            | LirOpcode::Div
            | LirOpcode::SetEq
            | LirOpcode::Call
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(CopyPropagation.name(), "cp");
    }
}