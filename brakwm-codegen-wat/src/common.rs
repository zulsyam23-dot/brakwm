use brakwm_ir_lir::lir::{LirType, VirtReg};

/// Memory layout constants shared by WAT and WASM backends.
pub const DATA_BASE: u32 = 0x400; // strings start at 1KiB
pub const STACK_BASE: u32 = 0x4000; // 16KiB reserved stack region
pub const MIN_PAGES: u32 = 2;

pub fn wasm_type(t: &LirType) -> &'static str {
    match t {
        LirType::I32 | LirType::Bool | LirType::Named(_) | LirType::Ptr(_) => "i32",
        LirType::I64 => "i64",
        LirType::F32 => "f32",
        LirType::F64 => "f64",
        LirType::String => "i32",
        LirType::Void => "i32",
    }
}

/// WASM value type tag for locals/params (used by binary encoder).
pub fn wasm_type_tag(t: &LirType) -> u8 {
    match wasm_type(t) {
        "i32" => 0x7F,
        "i64" => 0x7E,
        "f32" => 0x7D,
        "f64" => 0x7C,
        _ => 0x7F,
    }
}

/// Wasm reg name for a virtual register.
pub fn reg_name(r: VirtReg) -> String {
    format!("$r{r}")
}