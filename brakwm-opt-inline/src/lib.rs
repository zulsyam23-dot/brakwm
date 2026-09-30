use std::collections::HashMap;
use brakwm_core::Result;
use brakwm_ir_lir::lir::{LirInst, LirOpcode, LirOperand, LirProgram};
use brakwm_opt_traits::LirOptimizationPass;

pub struct Inlining;

impl LirOptimizationPass for Inlining {
    fn name(&self) -> &'static str {
        "inline"
    }

    fn run(&self, mut program: LirProgram) -> Result<LirProgram> {
        // Inline functions that are called exactly once and are small single
        // blocks (leaf, parameterless). Recursion-insensitive.
        inline_small_single_site(&mut program);
        Ok(program)
    }
}

fn inline_small_single_site(program: &mut LirProgram) {
    let call_counts = count_calls(program);
    let callee_bodies: HashMap<String, Vec<LirInst>> = program
        .functions
        .iter()
        .filter(|f| f.name != "main")
        .filter(|f| call_counts.get(&f.name).copied() == Some(1))
        .filter(|f| f.blocks.len() == 1 && f.params.is_empty())
        .filter(|f| f.blocks[0].insts.len() <= 8)
        .map(|f| (f.name.clone(), f.blocks[0].insts.clone()))
        .collect();

    if callee_bodies.is_empty() {
        return;
    }

    for func in &mut program.functions {
        for block in &mut func.blocks {
            let mut inlined: Vec<LirInst> = Vec::with_capacity(block.insts.len());
            for inst in &block.insts {
                if inst.opcode == LirOpcode::Call {
                    if let Some(LirOperand::Label(name)) = inst.operands.first() {
                        if let Some(body) = callee_bodies.get(name) {
                            // Skip a Ret pop of the call's result; splice body.
                            for sub in body.iter().filter(|i| i.opcode != LirOpcode::Ret) {
                                inlined.push(sub.clone());
                            }
                            continue;
                        }
                    }
                }
                inlined.push(inst.clone());
            }
            block.insts = inlined;
        }
    }

    // Drop now-dead trivially inlined callees.
    let still_used: HashMap<String, usize> = count_calls(program);
    if callee_bodies.keys().all(|c| still_used.get(c.as_str()).copied().unwrap_or(0) == 0) {
        let dead: Vec<String> = callee_bodies.keys().cloned().collect();
        program.functions.retain(|f| f.name == "main" || !dead.contains(&f.name));
    }
}

fn count_calls(program: &LirProgram) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for func in &program.functions {
        for block in &func.blocks {
            for inst in &block.insts {
                if let Some(LirOperand::Label(name)) = inst.operands.first() {
                    if inst.opcode == LirOpcode::Call {
                        *counts.entry(name.clone()).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn name_ok() {
        assert_eq!(Inlining.name(), "inline");
    }
}