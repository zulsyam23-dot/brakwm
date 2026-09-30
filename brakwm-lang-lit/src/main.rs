use brakwm_codegen_traits::CodegenBackend;
use brakwm_codegen_wasm::WasmCodegen;
use brakwm_core::Result;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: brakwm-lit <file.lit>");
        std::process::exit(2);
    }
    let path = &args[1];
    let src = std::fs::read_to_string(path)?;
    // .lit files are embedded as `entry LitExpr` on the HIR level; for the
    // milestone the tool wraps the expression inside a synthetic `main`.
    let wrapped = format!("fn main() -> i32 {{ {src} }}");
    let sm = brakwm_core::SourceMap::new(path, &wrapped);
    let mut parser = brakwm_frontend::parser::Parser::new();
    let ast = parser.parse_source(&sm)?;
    let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast)?;
    let mut ml = brakwm_ir_mir::lower::MirLower::new();
    let mir = ml.lower(hir)?;
    let mut ll = brakwm_ir_lir::lower::LirLower::new();
    ll.set_file_id(0);
    let lir = ll.lower(mir);
    let codegen = WasmCodegen::new();
    let bytes = codegen.emit(&lir)?;
    let out = path.trim_end_matches(".lit").to_string() + ".wasm";
    std::fs::write(&out, &bytes)?;
    println!("wrote {}", out);
    Ok(())
}