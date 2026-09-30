# brakwm-ir-ast

Abstract Syntax Tree, yaitu representasi closest-to-source dari program BrakWM.

## Struktur Data

- **Program & Item**: `Program` berisi daftar `Item`, yang mencakup `FnDef`, `StructDef`, `EnumDef`, `ConstDef`, `Let` (global), dan `ExternFn`. Visibilitas item dinyatakan lewat enum `Visibility`.
- **Fungsi**: `FnDef` memuat nama, daftar `Param`, tipe balik, dan `Block` tubuh. `ExternFn` menandai fungsi yang akan diselesaikan linker sebagai simbol eksternal.
- **Blok & Statement**: `Block` memuat daftar `Stmt` dan `tail: Option<Box<Expr>>`. Field `tail` inilah yang mewakili nilai balik suatu blok, dan menjadi tempat ekspresi di akhir fungsi disimpan.
- **Ekspresi**: `Expr` mencakup literal, identifier, binary/unary operation, pemanggilan fungsi, composite (binary/unary), field access, assignment, `if`, `while`, `for`, `match`, dan blok.
- **Pola & Literal**: `Pattern` untuk deconstructuring pada `match` dan parameter; `Literal` untuk nilai konstan. Operator berada di `BinOp`.

## Peran dalam Pipeline

AST adalah output `brakwm-frontend` dan input `brakwm-ir-hir`. Node AST sengaja dibuat sedekat mungkin dengan bentuk tulisan pengguna, sehingga `#[derive(Serialize, Deserialize)]` dapat dipakai `brakwm-bitcode` untuk menyimpan AST di cache tanpa kehilangan informasi.

Semua node menyimpan `Span` agar diagnostic dapat menunjuk lokasi persis pada tahap yang lebih akhir.
