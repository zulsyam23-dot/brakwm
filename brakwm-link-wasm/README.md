# brakwm-link-wasm

Linker WASM: memvalidasi modul hasil codegen dan memastikan entry point tersedia sebelum modul dianggap siap jalankan. Menggantikan posisi `brak-link-native` pada toolkit asli.

## API Utama

- **`WasmLinker::new()`**: linker dengan entry point `main` sebagai default.
- **`WasmLinker::without_start()`**: varian yang tidak mewajibkan keberadaan entry point, untuk modul yang tidak dimaksudkan dijalankan sebagai program.
- **`link(objects, entry, base_addr)`**: memverifikasi modul dan mengembalikan `LinkerOutput`.

## Validasi

Setiap object yang masuk diperiksa lebih dulu:

- Panjang data minimal delapan byte.
- Empat byte pertama harus berupa magic `\0asm`.
- Empat byte berikutnya harus versi modul `1 0 0 0`.

Setelah itu linker memastikan entry point tersedia. Entry point `main` diterima, dan `_start` juga diterima sebagai gantinya karena itulah entry point standar WASI.

## Batas Saat Ini

- **Multi-object belum didukung**: linker menerima tepat satu object yang sudah menjadi modul lengkap. Menggabungkan beberapa modul masih dikembalikan sebagai error.
- **Validasi struktural**: pemeriksaan dilakukan pada tingkat struktur modul, bukan terhadap integritas semantik seluruh program.

## Penggunaan

```rust
use brakwm_link_wasm::WasmLinker;
use brakwm_link_traits::{LinkerBackend, ObjectFile};

let out = WasmLinker::new().link(&[obj], "main", 0)?;
std::fs::write("program.wasm", out.data)?;
```
