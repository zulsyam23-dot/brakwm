# brakwm-codegen-wasm

Backend codegen yang mengubah LIR langsung menjadi bytecode modul WebAssembly biner, tanpa assembler eksternal. Menggantikan posisi `brak-codegen-obj` pada toolkit asli.

## Modul

### `encode.rs` - Primitif Encoding

Berisi encoder LEB128 dan pembantu section yang dipakai seluruh emitter:

- `leb_u32`, `leb_i32`, `leb_i64` untuk integer unsigned dan signed.
- `leb_f32`, `leb_f64` untuk konstanta floating point IEEE-754 little-endian.
- `section(id, payload, out)` menulis satu section WASM beserta panjangnya.
- `write_name` menulis nama yang diberi prefiks panjang.
- `Wat2WasmLike` menyatukan konvensi nama biner dan opcode yang sama antara keluaran teks WAT dan modul biner, sehingga keduanya dapat dibandingkan langsung.

### `emit.rs` - Emitter Modul

`emit_module_bytes(program: &LirProgram) -> Result<Vec<u8>>` menyusun modul WASM lengkap: magic, section tipe, import, fungsi, memori, global, export, kode, dan data.

Helper yang menyederhanakan pembacaan: `vt` memetakan `LirType` ke tag biner WASM, dan dipakai ulang untuk tipe fungsi, parameter, dan hasil.

## WASI Entry Point

Ketika program punya fungsi `main` dan belum mengekspor `_start` sendiri, emitter menyintesis:

- Import `wasi_snapshot_preview1.proc_exit` dengan tipe `(i32) -> ()`.
- Fungsi `_start` bertipe `() -> ()` yang memanggil `main`, lalu meneruskan hasilnya ke `proc_exit`. `main` bertipe void menghasilkan exit code 0; parameter `main` diisi dengan konstanta nol.
- Export `_start` selain export `memory` dan seluruh fungsi program.

Modul yang tidak punya `main` tidak menerima `_start` sintetis, dan program yang mendeklarasikan `proc_exit` sendiri akan memakai import miliknya, bukan hasil sintesis.

## Dispatch Program Counter

CFG dipetakan ke struktur dispatch berbasis program counter, sama seperti backend WAT: variabel `$__pc`, `block` bersarang dalam satu `loop`, dan `br_table` untuk perpindahan antar blok. Fungsi satu blok melewati dispatch sama sekali.

## Penggunaan

```rust
use brakwm_codegen_wasm::WasmCodegen;
use brakwm_codegen_traits::CodegenExecutable;

let bytes = WasmCodegen::new().generate(&lir)?;
```

## Catatan

Modul yang dihasilkan divalidasi dengan `WebAssembly.compile()` pada Node.js, dan dijalankan melalui harness WASI di `brakwm-tool`.
