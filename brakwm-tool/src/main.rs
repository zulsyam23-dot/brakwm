use std::path::{Path, PathBuf};
use clap::{Parser, Subcommand};
use brakwm_core::{Result, SourceMap};
use brakwm_codegen_traits::CodegenBackend;
use brakwm_ir_lir::lir::LirProgram;
use brakwm_link_traits::{LinkerBackend, ObjectFile};

mod runner;

#[derive(Parser)]
#[command(name = "brakwm", about = "BrakWM: a WASM-targeted compiler toolchain")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile a .brk source file to a .wasm module.
    Build {
        /// Input .brk file
        input: PathBuf,
        /// Output .wasm path (default: <input>.wasm)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Optimization level (0-3)
        #[arg(short = 'O', long, default_value_t = 0)]
        opt: u8,
    },
    /// Dump IR at a given stage (ast|hir|mir|lir|wat).
    EmitIr {
        input: PathBuf,
        /// IR stage to print
        stage: String,
    },
    /// Build and run under the Node WASI harness.
    Run {
        input: PathBuf,
        /// Extra args passed to the program
        #[arg(trailing_var_arg = true)]
        args: Vec<String>,
    },
    /// Link one or more object files (.obj) into a .wasm executable.
    Link {
        inputs: Vec<PathBuf>,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, default_value = "main")]
        entry: String,
    },
}

fn main() {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn run(cli: Cli) -> Result<i32> {
    match cli.command {
        Command::Build {
            input,
            output,
            opt,
        } => {
            let wasm = build(&input, opt)?;
            let out = output.unwrap_or_else(|| default_output(&input));
            std::fs::write(&out, &wasm)?;
            eprintln!("[brakwm] wrote {}", out.display());
            Ok(0)
        }
        Command::EmitIr { input, stage } => {
            let src = std::fs::read_to_string(&input)?;
            let filename = input.to_string_lossy().to_string();
            // `ast` and `hir` need the pre-MIR pipeline, so handle them here.
            match stage.as_str() {
                "ast" => {
                    let sm = SourceMap::new(&filename, &src);
                    let ast = brakwm_frontend::parser::Parser::new().parse_source(&sm)?;
                    println!("{ast}");
                    return Ok(0);
                }
                "hir" => {
                    let sm = SourceMap::new(&filename, &src);
                    let ast = brakwm_frontend::parser::Parser::new().parse_source(&sm)?;
                    let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast)?;
                    println!("{hir}");
                    return Ok(0);
                }
                "mir" => {
                    let sm = SourceMap::new(&filename, &src);
                    let ast = brakwm_frontend::parser::Parser::new().parse_source(&sm)?;
                    let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast)?;
                    let mir = brakwm_ir_mir::lower::MirLower::new().lower(hir)?;
                    println!("{mir}");
                    return Ok(0);
                }
                _ => {}
            }
            let lir = compile_to_lir(&src, &filename, 0)?;
            print_stage(&stage, &lir)?;
            Ok(0)
        }
        Command::Run { input, args } => {
            let wasm = build(&input, 0)?;
            let tmp = std::env::temp_dir().join(format!(
                "brakwm_run_{}.wasm",
                std::process::id()
            ));
            std::fs::write(&tmp, &wasm)?;
            let code = runner::run_node(&tmp, &args)?;
            let _ = std::fs::remove_file(&tmp);
            Ok(code)
        }
        Command::Link {
            inputs,
            output,
            entry,
        } => {
            let mut objects = Vec::new();
            for p in &inputs {
                objects.push(ObjectFile {
                    name: p.to_string_lossy().to_string(),
                    data: std::fs::read(p)?,
                });
            }
            let linker = brakwm_link_wasm::WasmLinker::new();
            let out = LinkerBackend::link(&linker, &objects, &entry, 0)?;
            std::fs::write(&output, &out.data)?;
            eprintln!("[brakwm] linked {} -> {}", inputs.len(), output.display());
            Ok(0)
        }
    }
}

fn default_output(input: &Path) -> PathBuf {
    let mut s = input.to_string_lossy().to_string();
    if let Some(pos) = s.rfind(".brk") {
        s.truncate(pos);
    }
    s.push_str(".wasm");
    PathBuf::from(s)
}

fn build(input: &Path, opt: u8) -> Result<Vec<u8>> {
    let src = std::fs::read_to_string(input)?;
    let lir = compile_to_lir(&src, &input.to_string_lossy(), opt)?;

    let wat = brakwm_codegen_wat::WatCodegen::new().emit(&lir)?;
    if std::env::var("BRAKWM_DEBUG_WAT").is_ok() {
        eprintln!("{}", String::from_utf8_lossy(&wat));
    }

    let codegen = brakwm_codegen_wasm::WasmCodegen::new();
    let obj_data = codegen.emit(&lir)?;
    let linker = brakwm_link_wasm::WasmLinker::new();
    let out = LinkerBackend::link(
        &linker,
        &[ObjectFile {
            name: "module".into(),
            data: obj_data,
        }],
        "main",
        0,
    )?;
    Ok(out.data)
}

fn compile_to_lir(src: &str, filename: &str, opt: u8) -> Result<LirProgram> {
    let sm = SourceMap::new(filename, src);
    let mut parser = brakwm_frontend::parser::Parser::new();
    let ast = parser.parse_source(&sm)?;
    let hir = brakwm_ir_hir::lower::HirLower::new().lower(ast)?;
    let mut ml = brakwm_ir_mir::lower::MirLower::new();
    let mir = ml.lower(hir)?;
    let mut ll = brakwm_ir_lir::lower::LirLower::new();
    ll.set_file_id(0);
    let mut lir = ll.lower(mir);

    if opt > 0 {
        let mut pm = brakwm_opt_traits::PassManager::default()
            .with_iterations(opt as usize)
            .with_verbose(std::env::var("BRAKWM_VERBOSE").is_ok());
        pm.add_pass(Box::new(brakwm_opt_fold::ConstantFolding));
        pm.add_pass(Box::new(brakwm_opt_cp::CopyPropagation));
        pm.add_pass(Box::new(brakwm_opt_gvn::GlobalValueNumbering));
        pm.add_pass(Box::new(brakwm_opt_jt::JumpThreading));
        pm.add_pass(Box::new(brakwm_opt_dce::DeadCodeElimination));
        lir = pm.run(lir)?;
    }
    Ok(lir)
}

fn print_stage(stage: &str, lir: &LirProgram) -> Result<()> {
    match stage {
        "lir" => {
            println!("{lir}");
        }
        "wat" => {
            let wat = brakwm_codegen_wat::WatCodegen::new().emit(lir)?;
            print!("{}", String::from_utf8_lossy(&wat));
        }
        "stats" => {
            println!("functions: {}", lir.functions.len());
            println!("strings: {}", lir.string_table.len());
            for f in &lir.functions {
                println!("  {} blocks={} regs={}", f.name, f.blocks.len(), f.reg_count);
            }
        }
        other => {
            return Err(format!(
                "unknown stage '{other}' (expected: lir|wat|stats)"
            )
            .into());
        }
    }
    Ok(())
}