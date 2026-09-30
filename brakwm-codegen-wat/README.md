# brakwm-codegen-wat

Backend codegen yang mengubah LIR menjadi teks WebAssembly Assembly (WAT). Menggantikan posisi `brak-codegen-asm` pada toolkit asli.

## API Utama

- **`WatCodegen`**: mengimplementasikan `CodegenBackend` dan menghasilkan teks WAT.
- **`emit_module(program: &LirProgram) -> Result<String>`**: fungsi utama yang menurunkan LIR menjadi string WAT.
- **Konstanta layout**: `DATA_BASE = 0x400`, `STACK_BASE = 0x4000`, dan `MIN_PAGES = 2` menentukan tata letak memori modul.
- **Helper**: `wasm_type` dan `wasm_type_tag` memetakan `LirType` ke nama dan tag biner WASM; `reg_name` memberi nama variabel lokal yang stabil untuk hasil keluaran.

## Model Eksekusi

Karena WebAssembly tidak memiliki stack call native, LIR/CFG dipetakan ke struktur dispatch berbasis program counter:

- Variabel lokal `$__pc` menyimpan indeks blok yang sedang dieksekusi.
- Seluruh basic block dibungkus `block` bersarang di dalam satu `loop`.
- `br_table` melompat ke blok yang sesuai dengan nilai `$__pc`.
- Variabel global mutable `$__brakwm_sp` berfungsi sebagai pointer stack, dipakai untuk `alloca` dan akses field struct.
- Fungsi yang hanya punya satu blok tidak memerlukan dispatch, sehingga blok itu langsung menjadi badan fungsi.

## Penggunaan

```rust
use brakwm_codegen_wat::codegen::WatCodegen;
use brakwm_codegen_traits::CodegenBackend;

let text = WatCodegen::new().generate(&lir)?;
println!("{text}");
```

## Kegunaan

Output WAT berguna untuk inspeksi dan debug: bentuk CFG, urutan register, serta konstanta layout memori dapat dibaca langsung. Backend biner di `brakwm-codegen-wasm` menghasilkan modul yang sama secara semantik, tetapi langsung dapat dieksekusi.
