use brakwm_core::Result;
use brakwm_ir_lir::lir::LirProgram;

/// Expected output of a sample: exit code / printed lines.
#[derive(Debug, Clone)]
pub struct SampleExpectation {
    pub exit_code: i32,
    pub stdout: String,
}

/// Driver that compiles a LIR program and evaluates it through a backend.
pub trait SampleRunner: Send + Sync {
    /// Compile + run the program; return observed exit code and stdout.
    fn run(&self, lir: &LirProgram) -> Result<(i32, String)>;
}

/// Tester that discovers samples, compiles them, and checks expectations.
pub struct TestRunner {
    runner: Box<dyn SampleRunner>,
    pub verbose: bool,
}

impl TestRunner {
    pub fn new(runner: Box<dyn SampleRunner>) -> Self {
        Self {
            runner,
            verbose: false,
        }
    }

    pub fn run_sample(&self, lir: &LirProgram, expected: &SampleExpectation) -> Result<bool> {
        let (code, stdout) = self.runner.run(lir)?;
        let ok = code == expected.exit_code && stdout == expected.stdout;
        if self.verbose || !ok {
            println!(
                "  exit: {} (expected {})  stdout: {:?} (expected {:?})",
                code, expected.exit_code, stdout, expected.stdout
            );
        }
        Ok(ok)
    }
}

/// Build a `main`-only program returning a constant exit code (used for
/// minimal sample smoke tests without a full runner).
pub fn constant_exit_lir(code: i64) -> LirProgram {
    use brakwm_ir_lir::lir::*;
    let mut prog = LirProgram {
        functions: vec![],
        extern_functions: vec![],
        structs: vec![],
        enums: vec![],
        string_table: vec![],
        files: vec!["sample.brk".to_string()],
    };
    prog.functions.push(LirFunction {
        name: "main".to_string(),
        params: vec![],
        ret_ty: LirType::I32,
        reg_types: vec![LirType::I32],
        reg_count: 1,
        blocks: vec![LirBlock {
            id: 0,
            name: "entry".to_string(),
            insts: vec![
                LirInst::new(LirOpcode::Mov)
                    .with_dest(0)
                    .with_op(LirOperand::ImmI64(code)),
                LirInst::new(LirOpcode::Ret).with_op(LirOperand::Reg(0)),
            ],
            span: brakwm_core::Span::default(),
        }],
        span: brakwm_core::Span::default(),
    });
    prog
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_constant_program() {
        let lir = constant_exit_lir(42);
        assert_eq!(lir.functions.len(), 1);
        assert_eq!(lir.functions[0].name, "main");
    }
}