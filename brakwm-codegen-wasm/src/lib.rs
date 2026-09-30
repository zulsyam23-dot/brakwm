use brakwm_core::Result;
use brakwm_codegen_traits::CodegenBackend;
use brakwm_ir_lir::lir::LirProgram;

pub mod encode;
pub mod emit;

pub use encode::{leb_u32, leb_i32, leb_i64, leb_f32, leb_f64, section, Wat2WasmLike};

/// WASM binary backend: LIR -> .wasm module bytes.
pub struct WasmCodegen;

impl WasmCodegen {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WasmCodegen {
    fn default() -> Self {
        Self::new()
    }
}

impl CodegenBackend for WasmCodegen {
    fn name(&self) -> &'static str {
        "wasm"
    }

    fn emit(&self, program: &LirProgram) -> Result<Vec<u8>> {
        emit::emit_module_bytes(program)
    }
}