use std::collections::HashMap;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram, VirtReg};
use brakwm_opt_traits::LirOptimizationPass;

pub struct ConstantFolding;

impl LirOptimizationPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "fold"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        for func in &mut program.functions {
            fold(func);
        }
        Ok(program)
    }
}

/// Fold constant expressions within each block, tracking known register
/// values intra-block (constants only; no memory).
fn fold(func: &mut brakwm_ir_lir::lir::LirFunction) {
    for block in &mut func.blocks {
        let mut known: HashMap<VirtReg, i64> = HashMap::new();
        let mut new_insts = Vec::with_capacity(block.insts.len());

        for inst in &block.insts {
            let mut new_inst = inst.clone();
            for op in new_inst.operands.iter_mut() {
                if let LirOperand::Reg(r) = op {
                    if let Some(v) = known.get(r) {
                        *op = LirOperand::ImmI64(*v);
                    }
                }
            }

            if let Some(d) = new_inst.dest {
                let folds = fold_binop(&new_inst);
                if let Some(v) = folds {
                    known.insert(d, v);
                    let folded = LirInst::new(LirOpcode::Mov)
                        .with_dest(d)
                        .with_op(LirOperand::ImmI64(v))
                        .with_debug(new_inst.debug)
                        .with_file(new_inst.file_id);
                    // Keep the original semantics hidden by a mov of constant.
                    new_insts.push(folded);
                    continue;
                }
            }
            new_insts.push(new_inst);
        }
        block.insts = new_insts;
    }
}

fn fold_binop(inst: &LirInst) -> Option<i64> {
    if !matches!(
        inst.opcode,
        LirOpcode::Add
            | LirOpcode::Sub
            | LirOpcode::Mul
            | LirOpcode::Div
            | LirOpcode::Mod
            | LirOpcode::And
            | LirOpcode::Or
            | LirOpcode::Xor
            | LirOpcode::Shl
            | LirOpcode::Shr
    ) {
        return None;
    }
    let a = match inst.operands.first() {
        Some(LirOperand::ImmI64(v)) => *v,
        _ => return None,
    };
    let b = match inst.operands.get(1) {
        Some(LirOperand::ImmI64(v)) => *v,
        _ => return None,
    };
    match inst.opcode {
        LirOpcode::Add => Some(a.wrapping_add(b)),
        LirOpcode::Sub => Some(a.wrapping_sub(b)),
        LirOpcode::Mul => Some(a.wrapping_mul(b)),
        LirOpcode::Div => (b != 0).then(|| a.wrapping_div(b)),
        LirOpcode::Mod => (b != 0).then(|| a.wrapping_rem(b)),
        LirOpcode::And => Some(a & b),
        LirOpcode::Or => Some(a | b),
        LirOpcode::Xor => Some(a ^ b),
        LirOpcode::Shl => (b < 64).then(|| a.wrapping_shl(b as u32)),
        LirOpcode::Shr => (b < 64).then(|| a.wrapping_shr(b as u32)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(ConstantFolding.name(), "fold");
    }

    #[test]
    fn folds_add() {
        let op = LirOpcode::Add;
        let inst = LirInst::new(op)
            .with_dest(3)
            .with_op(LirOperand::ImmI64(2))
            .with_op(LirOperand::ImmI64(3));
        assert_eq!(fold_binop(&inst), Some(5));
    }
}