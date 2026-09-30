# brakwm-codegen-traits

Trait bersama untuk backend codegen, sehingga codegen WAT dan codegen WASM biner dapat dipertukarkan tanpa mengubah pipeline.

## Trait

- **`CodegenBackend`**: trait dasar untuk backend yang mengubah LIR menjadi satu representasi output. Backend melaporkan nama, format yang dihasilkan, dan apakah ia dapat mengonversi ke output biner.
- **`CodegenExecutable`**: trait turunan untuk backend yang juga dapat menghasilkan modul siap jalankan.

## Backend yang Mengimplementasikan

- `brakwm-codegen-wat` mengimplementasikan `CodegenBackend` dan menghasilkan teks WAT.
- `brakwm-codegen-wasm` mengimplementasikan `CodegenExecutable` dan menghasilkan bytecode `.wasm` biner.

## Penggunaan

```rust
use brakwm_codegen_traits::CodegenBackend;
use brakwm_codegen_wasm::WasmCodegen;

let backend = WasmCodegen::new();
let bytes = backend.generate(&lir)?;
```

## Catatan

Trait ini sengaja dibuat tipis. Logika penurunan yang sama dikumpulkan di masing-masing backend; yang dikumpulkan di sini hanya kontrak dan nama format, agar menambah backend baru tidak memaksa pipeline berubah.
