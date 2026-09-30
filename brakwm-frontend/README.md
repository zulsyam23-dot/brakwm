# brakwm-frontend

Lexer dan Parser untuk mengubah source `.brk` menjadi AST.

## Komponen

- **Lexer**: `AsciiLexer` mengimplementasikan trait `BrakLexer` dan menghasilkan `Token` dari `SourceMap`. `TokenKind` mencakup identifier, keyword, literal numerik, dan operator. Lexer melewati BOM UTF-8 di awal file agar source yang dihasilkan editor tidak gagal di-parse.
- **Parser**: `Parser` menghasilkan `brakwm_ir_ast::Program` melalui `parse_source` (dari `SourceMap`) atau `parse` (dari slice token yang sudah di-lex).

## Tail Expression dan Parentesis

Blok mendukung ekspresi sebagai nilai balik. `parse_block` mencoba parse eksensi secara spekulatif: bila ekspresi itu berada di posisi terakhir blok, ekspresi tersebut menjadi `Block::tail`; bila tidak, posisi token dikembalikan dan ekspresi diparse sebagai statement biasa. Mekanisme ini membuat `if` dapat menjadi nilai di akhir fungsi sekaligus tetap diperlakukan sebagai statement bila diikuti tanda baca lain.

Statement dan ekspresi `if`/`while` juga boleh ditulis dengan maupun tanpa tanda kurung pembuka. Tanda kurung bersifat opsional agar penulisan ringkas dan gaya lama sama-sama valid.

## Penggunaan

```rust
use brakwm_core::SourceMap;
use brakwm_frontend::parser::Parser;

let sm = SourceMap::new("hello.brk", "fn main() -> i32 { 42 }");
let mut parser = Parser::new();
let program = parser.parse_source(&sm)?;
```

## Catatan

Frontend hanya menangani syntactic parsing. Pemeriksaan tipe, pelaboration nama, dan generasi MIR ditangani crate pada tahap berikutnya.
