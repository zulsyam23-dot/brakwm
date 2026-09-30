use brakwm_core::{Result, SourceMap};
use brakwm_codegen_traits::CodegenBackend;
use brakwm_ir_lir::lir::LirProgram;
use brakwm_codegen_wasm::WasmCodegen;
use brakwm_link_wasm::WasmLinker;
use brakwm_link_traits::{LinkerBackend, LinkerOutput, ObjectFile};

/// One-call convenience API: .brk source string -> final WASM bytes.
pub struct BrakWm {
    pub optimize: bool,
    pub emit_wat: bool,
}

impl Default for BrakWm {
    fn default() -> Self {
        Self {
            optimize: false,
            emit_wat: false,
        }
    }
}

impl BrakWm {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_optimize(mut self, on: bool) -> Self {
        self.optimize = on;
        self
    }

    pub fn with_wat(mut self, on: bool) -> Self {
        self.emit_wat = on;
        self
    }

    /// Full pipeline: source string -> vectors of WASM bytes.
    pub fn compile(&self, src: &str, filename: &str) -> Result<Vec<u8>> {
        let lir = self.lower(src, filename)?;
        let lir = self.run_opt(lir)?;
        let obj = self.emit(lir)?;
        let out = self.link(obj, "main")?;
        Ok(out.data)
    }

    pub fn lower(&self, src: &str, filename: &str) -> Result<LirProgram> {
        let sm = SourceMap::new(filename, src);
        let mut parser = brakwm_frontend::parser::Parser::new();
        let ast = parser.parse_source(&sm)?;
        let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast)?;
        let mut ml = brakwm_ir_mir::lower::MirLower::new();
        let mir = ml.lower(hir)?;
        let mut ll = brakwm_ir_lir::lower::LirLower::new();
        ll.set_file_id(0);
        Ok(ll.lower(mir))
    }

    pub fn run_opt(&self, lir: LirProgram) -> Result<LirProgram> {
        if !self.optimize {
            return Ok(lir);
        }
        let mut pm = brakwm_opt_traits::PassManager::default()
            .with_iterations(3)
            .with_verbose(false);
        pm.add_pass(Box::new(brakwm_opt_dce::DeadCodeElimination));
        pm.add_pass(Box::new(brakwm_opt_cp::CopyPropagation));
        pm.add_pass(Box::new(brakwm_opt_fold::ConstantFolding));
        pm.add_pass(Box::new(brakwm_opt_gvn::GlobalValueNumbering));
        pm.add_pass(Box::new(brakwm_opt_inline::Inlining));
        pm.run(lir)
    }

    pub fn emit(&self, lir: LirProgram) -> Result<ObjectFile> {
        if self.emit_wat {
            let wat = brakwm_codegen_wat::WatCodegen::new().emit(&lir)?;
            return Ok(ObjectFile {
                name: "out.wat".into(),
                data: wat,
            });
        }
        let codegen = WasmCodegen::new();
        let bytes = codegen.emit(&lir)?;
        Ok(ObjectFile {
            name: "out.wasm".into(),
            data: bytes,
        })
    }

    pub fn link(&self, obj: ObjectFile, entry: &str) -> Result<LinkerOutput> {
        let linker = WasmLinker::new();
        LinkerBackend::link(&linker, &[obj], entry, 0)
    }

    /// Compile + link + save to a path.
    pub fn build_file(&self, path: &str) -> Result<()> {
        let src = std::fs::read_to_string(path)?;
        let out = self.compile(&src, path)?;
        let wasm = path.trim_end_matches(".brk").to_string() + ".wasm";
        std::fs::write(&wasm, &out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compiles_hello() {
        let wm = BrakWm::new();
        let bytes = wm.compile("fn main() -> i32 { 42 }", "hello.brk").unwrap();
        assert!(bytes[0] == 0x00 && bytes[1] == 0x61 && bytes[2] == 0x73 && bytes[3] == 0x6d);
    }
}