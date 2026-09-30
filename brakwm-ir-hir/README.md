# brakwm-ir-hir

High-level IR: AST yang sudah dielaborasi dengan informasi tipe, siap untuk penurunan ke MIR.

## Struktur Data

- **Program & Item**: `HirProgram` berisi `HirItem`, yaitu `HirFunction`, `HirStruct`, `HirEnum`, `HirExternFunction`, dan `HirGlobalLet`.
- **Fungsi**: `HirFunction` memuat `HirParam` yang sudah bertipe, tipe balik, dan `HirBlock` tubuh.
- **Tipe**: `HirType` menyatakan tipe eksplisit program (`i32`, `i64`, `f32`, `f64`, `bool`, `void`, struct, dan enum). Setiap ekspresi dalam `HirExpr` menyertakan tipe hasil, sehingga validasi tipe dapat dilakukan di tahap ini.
- **Blok & Statement**: `HirBlock` memiliki `tail: Option<Box<HirExpr>>` untuk nilai balik, dan daftar `HirStmt`.
- **Ekspresi**: `HirExpr` menyimpan setiap node beserta `HirType` hasilnya, termasuk `if`, `while`, `match`, akses field, dan composite.

## Pipeline

Output `brakwm-frontend` menjadi input `brakwm-ir-mir`. Bentuk `HirBlock::tail` diteruskan apa adanya ke MIR, sehingga nilai balik blok dapat diproses sebagai ekspresi biasa pada tahap berikutnya.

## Penggunaan

```rust
use brakwm_core::SourceMap;
use brakwm_frontend::parser::Parser;
use brakwm_ir_hir::lower::HirLower;

let sm = SourceMap::new("t.brk", "fn main() -> i32 { 42 }");
let ast = Parser::new().parse_source(&sm)?;
let hir = HirLower::new().lower(ast)?;
```

## Catatan

Validasi tipe masih minimal pada tahap ini: HIR mencatat tipe yang dinyatakan dan diturunkan, tetapi error-checking komprehensif belum menjadi fokus. Pelaporan tipe bertumpu pada tahap ekspresi, dan belum ada tabel simbol terpusat.
