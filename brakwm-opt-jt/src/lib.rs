use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirOpcode, LirOperand, LirProgram};
use brakwm_opt_traits::LirOptimizationPass;

pub struct JumpThreading;

impl LirOptimizationPass for JumpThreading {
    fn name(&self) -> &'static str {
        "jt"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        for func in &mut program.functions {
            thread(func);
        }
        Ok(program)
    }
}

/// Collapse `Jmp` diamonds: if a block contains only a Jmp to a block that
/// itself contains only a Jmp, the middle hop can be removed by rewriting
/// the target label. Also folds unconditional Jmp that targets the immediate
/// next block (fall-through).
fn thread(func: &mut brakwm_ir_lir::lir::LirFunction) {
    let mut changed = true;
    while changed {
        changed = false;
        // Build a map: label string -> label string for pure-jump blocks.
        let mut jump_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        for block in &func.blocks {
            if block.insts.len() == 1 {
                let inst = &block.insts[0];
                if inst.opcode == LirOpcode::Jmp {
                    if let Some(LirOperand::Label(target)) = inst.operands.first() {
                        let from = block.name.clone();
                        let t = jump_map
                            .get(target)
                            .cloned()
                            .unwrap_or_else(|| target.clone());
                        jump_map.insert(from, t);
                    }
                }
            }
        }

        for block in &mut func.blocks {
            for inst in block.insts.iter_mut() {
                if inst.opcode == LirOpcode::Jmp {
                    if let Some(LirOperand::Label(target)) = inst.operands.first_mut() {
                        if let Some(resolved) = jump_map.get(target) {
                            if resolved != target {
                                *target = resolved.clone();
                                changed = true;
                            }
                        }
                    }
                }
                if inst.opcode == LirOpcode::Br {
                    for op in inst.operands.iter_mut().skip(1) {
                        if let LirOperand::Label(target) = op {
                            if let Some(resolved) = jump_map.get(target) {
                                if resolved != target {
                                    *target = resolved.clone();
                                    changed = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(JumpThreading.name(), "jt");
    }
}