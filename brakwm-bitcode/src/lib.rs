use brakwm_core::Result;
use brakwm_ir_ast::ast::Program;
use brakwm_ir_hir::hir::HirProgram;
use brakwm_ir_lir::lir::LirProgram;
use brakwm_ir_mir::mir::MirProgram;

/// Serialize/deserialize IR stages to JSON (the "bitcode" equivalent).
pub fn write_ast(p: &Program) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(p)?)
}

pub fn read_ast(bytes: &[u8]) -> Result<Program> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn write_hir(p: &HirProgram) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(p)?)
}

pub fn read_hir(bytes: &[u8]) -> Result<HirProgram> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn write_mir(p: &MirProgram) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(p)?)
}

pub fn read_mir(bytes: &[u8]) -> Result<MirProgram> {
    Ok(serde_json::from_slice(bytes)?)
}

pub fn write_lir(p: &LirProgram) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(p)?)
}

pub fn read_lir(bytes: &[u8]) -> Result<LirProgram> {
    Ok(serde_json::from_slice(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lir_roundtrip() {
        let lir = brakwm_test::constant_exit_lir(7);
        let bytes = write_lir(&lir).unwrap();
        let back = read_lir(&bytes).unwrap();
        assert_eq!(back.functions.len(), 1);
        assert_eq!(back.functions[0].name, "main");
    }
}
