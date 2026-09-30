use brakwm_core::Result;
use brakwm_link_traits::{LinkerBackend, ObjectFile};
use brakwm_link_wasm::WasmLinker;

/// Archive format: sequence of length-prefixed object blobs followed by a
/// symbol directory. Multi-object WASM archives are linked left-to-right:
/// later objects "re-export" earlier exports for the symbol table.
#[derive(Debug, Clone)]
pub struct Archive {
    pub objects: Vec<ObjectFile>,
}

impl Default for Archive {
    fn default() -> Self {
        Self::new()
    }
}

impl Archive {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    pub fn add_object(&mut self, obj: ObjectFile) {
        self.objects.push(obj);
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn symbols(&self) -> Vec<String> {
        // Symbol directory mirrors object names (exported entry hook).
        self.objects.iter().map(|o| o.name.clone()).collect()
    }

    /// Serialize to the .brka container (for ISM/extended workflows).
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        let mut out = Vec::new();
        out.extend_from_slice(b"BRKMMA00");
        for obj in &self.objects {
            out.extend_from_slice(&(obj.data.len() as u32).to_le_bytes());
            out.extend_from_slice(&obj.data);
        }
        out.extend_from_slice(&(self.objects.len() as u32).to_le_bytes());
        Ok(out)
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self> {
        if !data.starts_with(b"BRKMMA00") {
            return Err("invalid archive magic".into());
        }
        let mut off = 8usize;
        let mut objects = Vec::new();
        let mut count_bytes = [0u8; 4];
        while off + 4 <= data.len() && (off + 4) <= data.len().saturating_sub(4) {
            if off + 4 > data.len() {
                break;
            }
            count_bytes.copy_from_slice(&data[off..off + 4]);
            let n = u32::from_le_bytes(count_bytes) as usize;
            if n == 0 {
                break;
            }
            off += 4;
            if off + n > data.len() {
                return Err("archive truncated".into());
            }
            objects.push(ObjectFile {
                name: format!("o{}", objects.len()),
                data: data[off..off + n].to_vec(),
            });
            off += n;
        }
        Ok(Self { objects })
    }

    /// Link all contained objects to a single WASM module.
    pub fn link_with(&self, linker: &WasmLinker, entry: &str, base_addr: u64) -> Result<brakwm_link_traits::LinkerOutput> {
        LinkerBackend::link(linker, &self.objects, entry, base_addr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let mut a = Archive::new();
        a.add_object(ObjectFile {
            name: "a.wasm".into(),
            data: vec![1, 2, 3],
        });
        let bytes = a.to_bytes().unwrap();
        let b = Archive::from_bytes(&bytes).unwrap();
        assert_eq!(b.objects.len(), 1);
        assert_eq!(b.objects[0].data, vec![1, 2, 3]);
    }
}