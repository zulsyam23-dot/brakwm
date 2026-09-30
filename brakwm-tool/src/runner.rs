use std::path::Path;
use brakwm_core::Result;

/// Run a .wasm module under Node's WASI preview1 and return the exit code.
/// The module should export the WASI entry point `_start`, which calls `main`
/// and routes the result through `wasi_snapshot_preview1.proc_exit`. With
/// `returnOnExit` the runtime reports that code instead of exiting, so the
/// result can be mapped onto the process exit status. Modules that only export
/// `main: () -> i32` fall back to invoking it directly.
pub fn run_node(wasm: &Path, args: &[String]) -> Result<i32> {
    let script = r#"
import { WASI } from 'node:wasi';
import { readFile } from 'node:fs/promises';

const wasmPath = process.argv[2];
const wasi = new WASI({
  version: 'preview1',
  args: ['brakwm', ...process.argv.slice(3)],
  env: process.env,
  preopens: {},
  returnOnExit: true,
});
const bytes = await readFile(wasmPath);
const module = await WebAssembly.compile(bytes);
const instance = await WebAssembly.instantiate(module, wasi.getImportObject());

if (typeof instance.exports._start === 'function') {
  // Real WASI entry point: main's result is routed through proc_exit.
  process.exit(wasi.start(instance) | 0);
} else if (typeof instance.exports.main === 'function') {
  process.exit(instance.exports.main() | 0);
} else {
  process.exit(0);
}
"#;

    let tmp_script = std::env::temp_dir().join(format!("brakwm_runner_{}.mjs", std::process::id()));
    std::fs::write(&tmp_script, script)?;

    let mut cmd = std::process::Command::new("node");
    cmd.arg(&tmp_script).arg(wasm);
    for a in args {
        cmd.arg(a);
    }
    let status = cmd.status();
    let _ = std::fs::remove_file(&tmp_script);
    let status = status?;
    Ok(status.code().unwrap_or(1))
}
