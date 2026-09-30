use std::collections::HashMap;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram, VirtReg};
use brakwm_opt_traits::LirOptimizationPass;

pub struct GlobalValueNumbering;

impl LirOptimizationPass for GlobalValueNumbering {
    fn name(&self) -> &'static str {
        "gvn"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        for func in &mut program.functions {
            value_number(func);
        }
        Ok(program)
    }
}

/// Intra-procedural GVN on a single pass per block: reuse computation of an
/// identical earlier expression stored in a register that is still live.
fn value_number(func: &mut brakwm_ir_lir::lir::LirFunction) {
    for block in &mut func.blocks {
        let mut seen: HashMap<String, VirtReg> = HashMap::new();
        let mut replacements: HashMap<VirtReg, VirtReg> = HashMap::new();
        let mut new_insts = Vec::with_capacity(block.insts.len());

        for inst in block.insts.clone() {
            let mut new_inst = inst.clone();
            for op in new_inst.operands.iter_mut() {
                if let LirOperand::Reg(r) = op {
                    if let Some(&orig) = replacements.get(r) {
                        *op = LirOperand::Reg(orig);
                    }
                }
            }

            if let Some(d) = new_inst.dest {
                if is_expression(&new_inst) {
                    let key = expr_key(&new_inst);
                    if let Some(&original) = seen.get(&key) {
                        replacements.insert(d, original);
                        // After propagation, rewrite directly to a Mov.
                        new_inst = LirInst::new(LirOpcode::Mov)
                            .with_dest(d)
                            .with_op(LirOperand::Reg(original))
                            .with_debug(new_inst.debug)
                            .with_file(new_inst.file_id);
                        new_insts.push(new_inst);
                        continue;
                    } else {
                        seen.insert(key, d);
                    }
                }
            }

            new_insts.push(new_inst);
        }

        // Apply replacement in later instructions of the block.
        let mut out = Vec::new();
        for inst in new_insts {
            let mut i = inst;
            for op in i.operands.iter_mut() {
                if let LirOperand::Reg(r) = op {
                    if let Some(&rep) = replacements.get(r) {
                        *op = LirOperand::Reg(rep);
                    }
                }
            }
            out.push(i);
        }
        block.insts = out;
    }
}

fn is_expression(inst: &LirInst) -> bool {
    matches!(
        inst.opcode,
        LirOpcode::Add | LirOpcode::Sub | LirOpcode::Mul | LirOpcode::Div | LirOpcode::Mod | LirOpcode::And | LirOpcode::Or | LirOpcode::Xor
    )
}

fn expr_key(inst: &LirInst) -> String {
    let mut s = format!("{:?}", inst.opcode);
    for op in &inst.operands {
        if let LirOperand::Reg(r) = op {
            s.push_str(&format!(" %r{r}"));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(GlobalValueNumbering.name(), "gvn");
    }
}