# brakwm-ir-lir

Low-level IR berbasis register virtual: bentuk final sebelum optimasi dan codegen.

## Struktur Data

- **Program**: `LirProgram` berisi fungsi, extern fungsi, metadata struct/enum, dan `string_table` untuk data statis.
- **Fungsi**: `LirFunction` memuat `params` (daftar `VirtReg`), `ret_ty`, `reg_types` yang memetakan setiap register ke `LirType`, dan daftar `LirBlock`. Metode `param_regs` dan `param_types` merupakan pintasan yang dipakai backend.
- **Blok & Instruksi**: `LirBlock` memuat `insts: Vec<LirInst>`. `LirInst` menggabungkan `LirOpcode`, register tujuan opsional, dan daftar `LirOperand` (register, konstanta, atau label blok).
- **Tipe**: `LirType` mencakup `I32`, `I64`, `F32`, `F64`, dan `Void`. Helper `is_float` dan `wasm_type` memudahkan backend WASM.
- **Metadata Agregat**: `LirStructMetadata` dan `LirEnumMetadata` menyimpan urutan field sehingga `GetField` dan `SetField` dapat dihitung offset-nya di backend. Enum disintesis sebagai struct `__enum_<Nama>` dengan field `$tag` ditambah payload.

## Dari MIR ke LIR

`LirLower` menerjemahkan tiap `MirInst` dan `MirTerminator` menjadi pasangan `LirOpcode` dan `LirOperand` yang sesuai. Terminator `Branch` menjadi `Br` dengan dua label, `Jump` menjadi `Jmp`, dan `Return` menjadi `Ret` beserta operand nilainya.

## Penggunaan

```rust
let mir = MirLower::new().lower(hir)?;
let lir = LirLower::new().lower(mir);
```

LIR adalah tempat seluruh pass optimasi bekerja, dan juga bentuk yang langsung dikonsumsi kedua backend codegen.
