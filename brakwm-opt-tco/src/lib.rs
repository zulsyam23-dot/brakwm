use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram};
use brakwm_opt_traits::LirOptimizationPass;

pub struct TailCallOptimization;

impl LirOptimizationPass for TailCallOptimization {
    fn name(&self) -> &'static str {
        "tco"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        convert_self_tail_calls(&mut program);
        Ok(program)
    }
}

fn convert_self_tail_calls(program: &mut LirProgram) {
    for func in &mut program.functions {
        let entry_label = LirOperand::Label(format!("block_{}", func.blocks[0].id));
        for block in &mut func.blocks {
            let n = block.insts.len();
            if n < 2 {
                continue;
            }
            let tail = &block.insts[n - 2];
            let ret = &block.insts[n - 1];
            if tail.opcode != LirOpcode::Call || ret.opcode != LirOpcode::Ret {
                continue;
            }
            match tail.operands.first() {
                Some(LirOperand::Label(name)) if name == &func.name => {}
                _ => continue,
            }
            // Only when the Ret uses the call's result register or the call
            // has no result. Conservative transformation.
            let call_result = tail.dest;
            let ret_uses_call_result = match ret.operands.first() {
                Some(LirOperand::Reg(r)) => call_result == Some(*r),
                None => true,
                _ => false,
            };
            if !ret_uses_call_result {
                continue;
            }

            // Transform: re-bind params from call args, then jump to entry.
            let mut new_insts = Vec::with_capacity(n);
            for (i, arg) in tail.operands.iter().skip(1).enumerate() {
                if let (Some(param), LirOperand::Reg(arg)) = (func.params.get(i), arg) {
                    new_insts.push(
                        LirInst::new(LirOpcode::Mov)
                            .with_dest(*param)
                            .with_op(LirOperand::Reg(*arg)),
                    );
                }
            }
            new_insts.push(LirInst::new(LirOpcode::Jmp).with_op(entry_label.clone()));
            block.insts = new_insts;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(TailCallOptimization.name(), "tco");
    }
}