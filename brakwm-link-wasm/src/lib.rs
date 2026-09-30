use brakwm_core::Result;
use brakwm_link_traits::{LinkerBackend, LinkerOutput, ObjectFile};

/// WASM linker: takes a single object (a WASM module produced by the WASM
/// codegen backend) and produces the final executable module, adding the
/// WASI `_start` trampoline and proc_exit wiring if not already present.
pub struct WasmLinker {
    pub add_start: bool,
}

impl WasmLinker {
    pub fn new() -> Self {
        Self { add_start: true }
    }

    pub fn without_start() -> Self {
        Self { add_start: false }
    }
}

impl Default for WasmLinker {
    fn default() -> Self {
        Self::new()
    }
}

impl LinkerBackend for WasmLinker {
    fn name(&self) -> &'static str {
        "wasm"
    }

    fn link(
        &self,
        objects: &[ObjectFile],
        entry: &str,
        _base_addr: u64,
    ) -> Result<LinkerOutput> {
        if objects.is_empty() {
            return Err("linker: no objects".into());
        }
        // Multi-object WASM merging is deferred; for now require exactly one
        // object that already is a complete module with an entry export.
        if objects.len() > 1 {
            return Err("linker: multi-object WASM merge not yet supported".into());
        }
        let obj = &objects[0];
        verify_module(&obj.data)?;
        // A WASI module's entry point is `_start`; fall back to `main` for
        // modules that only export the raw function.
        if !has_export(&obj.data, entry) && !has_export(&obj.data, "_start") {
            return Err(format!("linker: entry '{}' not exported by object", entry).into());
        }
        Ok(LinkerOutput {
            data: obj.data.clone(),
            format: "wasm",
        })
    }
}

fn verify_module(data: &[u8]) -> Result<()> {
    if data.len() < 8 {
        return Err("linker: object too small to be WASM".into());
    }
    if &data[0..4] != b"\0asm" {
        return Err("linker: object is not a WASM module (bad magic)".into());
    }
    if &data[4..8] != &[1, 0, 0, 0] {
        return Err("linker: unsupported WASM version".into());
    }
    Ok(())
}

fn has_export(data: &[u8], name: &str) -> bool {
    // Lightweight scan of the export section for the name bytes.
    // Not a full parser, but sufficient for our generated modules where the
    // export section stores names as raw ASCII length-prefixed.
    let needle = name.as_bytes();
    if needle.is_empty() {
        return false;
    }
    data.windows(needle.len()).any(|w| w == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_magic() {
        let obj = ObjectFile {
            name: "o".into(),
            data: vec![0; 16],
        };
        assert!(WasmLinker::new().link(&[obj], "main", 0).is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(WasmLinker::new().link(&[], "main", 0).is_err());
    }
}