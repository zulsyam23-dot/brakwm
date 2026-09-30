/// Minimal LEB128 + section writer helpers for the WASM binary encoder.

pub fn leb_u32(mut v: u32, out: &mut Vec<u8>) {
    loop {
        let mut byte = (v & 0x7f) as u8;
        v >>= 7;
        if v != 0 {
            byte |= 0x80;
        }
        out.push(byte);
        if v == 0 {
            break;
        }
    }
}

pub fn leb_i32(mut v: i32, out: &mut Vec<u8>) {
    loop {
        let byte = (v as u8) & 0x7f;
        v >>= 7;
        let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

pub fn leb_i64(mut v: i64, out: &mut Vec<u8>) {
    loop {
        let byte = (v as u8) & 0x7f;
        v >>= 7;
        let done = (v == 0 && byte & 0x40 == 0) || (v == -1 && byte & 0x40 != 0);
        if done {
            out.push(byte);
            break;
        }
        out.push(byte | 0x80);
    }
}

pub fn leb_f32(v: f32, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_le_bytes());
}

pub fn leb_f64(v: f64, out: &mut Vec<u8>) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Emit a section: `id (len payload) payload`.
pub fn section(id: u8, payload: &[u8], out: &mut Vec<u8>) {
    out.push(id);
    leb_u32(payload.len() as u32, out);
    out.extend_from_slice(payload);
}

/// Write a length-prefixed name (used by import/export/data).
pub fn write_name(name: &str, out: &mut Vec<u8>) {
    leb_u32(name.len() as u32, out);
    out.extend_from_slice(name.as_bytes());
}

/// Not a real converter; kept as a marker for future WAT->WASM tooling.
pub struct Wat2WasmLike;
