# brakwm-bitcode

Serialisasi IR dalam bentuk JSON, untuk menyimpan dan memulihkan hasil antar tahap pipeline.

## API

Setiap tahap IR memiliki sepasang fungsi tulis dan baca:

- `write_ast` dan `read_ast` untuk `brakwm_ir_ast::Program`.
- `write_hir` dan `read_hir` untuk `HirProgram`.
- `write_mir` dan `read_mir` untuk `MirProgram`.
- `write_lir` dan `read_lir` untuk `LirProgram`.

Seluruh fungsi mengembalikan `Result<Vec<u8>>` untuk penulisan dan `Result<T>` untuk pembacaan, memakai tipe error dari `brakwm-core`.

## Penggunaan

```rust
use brakwm_bitcode;

let bytes = brakwm_bitcode::write_mir(&mir)?;
let restored = brakwm_bitcode::read_mir(&bytes)?;
```

## Status

API ini bersifat eksperimental dan belum menjadi bagian dari pipeline CLI. Kegunaan utamanya adalah inspecting serta membandingkan IR antar tahap ketika menelusuri bug, serta menyimpan hasil lowering agar tidak perlu mengulang parsing.

Struktur data yang dapat diserialisasi bergantung pada `#[derive(Serialize, Deserialize)]` pada node IR. Bila suatu node diubah dan kehilangan derive tersebut, fungsi baca-tulis untuk tahap itu perlu disesuaikan.
