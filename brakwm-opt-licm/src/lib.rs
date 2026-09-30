use std::collections::HashSet;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram, VirtReg};
use brakwm_opt_traits::LirOptimizationPass;
use brakwm_opt_utils::{cfg_successors, loop_headers};

pub struct LoopInvariantCodeMotion;

impl LirOptimizationPass for LoopInvariantCodeMotion {
    fn name(&self) -> &'static str {
        "licm"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        for func in &mut program.functions {
            hoist(func);
        }
        Ok(program)
    }
}

/// For each loop, find instructions whose operands are all invariant w.r.t.
/// the loop (defined only outside the loop) and hoist them to the preheader.
fn hoist(func: &mut brakwm_ir_lir::lir::LirFunction) {
    let succ = cfg_successors(func);
    let headers = loop_headers(func, &succ);
    if headers.is_empty() {
        return;
    }

    // Compute a cheap loop body approximation: all blocks reachable from the
    // header without passing through the header again.
    for header in &headers {
        let mut body: HashSet<usize> = HashSet::new();
        let mut work = vec![*header];
        while let Some(b) = work.pop() {
            if !body.insert(b) {
                continue;
            }
            if let Some(edges) = succ.get(&b) {
                for e in edges {
                    if !body.contains(e) {
                        work.push(*e);
                    }
                }
            }
        }

        // Invariant = no instruction in body defines a value used as operand
        // here, and operands are not memory loads/stores.
        let mut defs_outside: HashSet<VirtReg> = HashSet::new();
        let mut defs: HashSet<VirtReg> = HashSet::new();
        for bi in 0..func.blocks.len() {
            let in_body = body.contains(&bi);
            for inst in &func.blocks[bi].insts {
                if let Some(d) = inst.dest {
                    if in_body {
                        defs.insert(d);
                    } else {
                        defs_outside.insert(d);
                    }
                }
            }
        }

        let mut hoisted: Vec<LirInst> = Vec::new();
        for &bi in body.iter() {
            let block = &func.blocks[bi];
            let mut kept = Vec::with_capacity(block.insts.len());
            for inst in &block.insts {
                let operands_regs: Vec<VirtReg> = inst
                    .operands
                    .iter()
                    .filter_map(|op| match op {
                        LirOperand::Reg(r) => Some(*r),
                        _ => None,
                    })
                    .collect();
                let invariant = !has_memory_op(inst)
                    && operands_regs.iter().all(|r| !defs.contains(r))
                    && operands_regs.iter().all(|r| defs_outside.contains(r) || !is_def_in_body(func, *r, &body));
                if invariant && operands_regs.iter().all(|r| !defs.contains(r)) {
                    if matches!(
                        inst.opcode,
                        LirOpcode::Add
                            | LirOpcode::Sub
                            | LirOpcode::Mul
                            | LirOpcode::And
                            | LirOpcode::Or
                            | LirOpcode::Xor
                    ) {
                        hoisted.push(inst.clone());
                        continue;
                    }
                }
                kept.push(inst.clone());
            }
            func.blocks[bi].insts = kept;
        }

        // Place hoisted instructions at the start of the header block.
        if !hoisted.is_empty() {
            let header_block = &func.blocks[*header];
            let mut merged = hoisted;
            merged.extend(header_block.insts.clone());
            func.blocks[*header].insts = merged;
        }
    }
}

fn is_def_in_body(
    func: &brakwm_ir_lir::lir::LirFunction,
    r: VirtReg,
    body: &HashSet<usize>,
) -> bool {
    for bi in 0..func.blocks.len() {
        if !body.contains(&bi) {
            continue;
        }
        for inst in &func.blocks[bi].insts {
            if inst.dest == Some(r) {
                return true;
            }
        }
    }
    false
}

fn has_memory_op(inst: &LirInst) -> bool {
    matches!(
        inst.opcode,
        LirOpcode::Load | LirOpcode::Store | LirOpcode::Call | LirOpcode::GetField | LirOpcode::SetField | LirOpcode::Alloca
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(LoopInvariantCodeMotion.name(), "licm");
    }
}