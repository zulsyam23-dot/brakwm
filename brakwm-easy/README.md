# brakwm-easy

Pipeline kompilasi siap pakai untuk pemetaan end-to-end dalam satu objek, tanpa perlu merangkai crate pipeline secara manual.

## API

- **`BrakWm::new()`**: membuat instance pipeline dengan setelan awal.
- **`with_optimize(on)`**: mengaktifkan atau menonaktifkan pipeline optimasi.
- **`with_wat(on)`**: mengaktifkan atau menonaktifkan keluaran WAT selain modul biner.
- **`lower(src, filename)`**: menjalankan parsing dan lowering sampai LIR.
- **`run_opt(lir)`**: menjalankan pipeline optimasi pada LIR.
- **`emit(lir)`**: melakukan codegen menjadi object modul.
- **`link(obj, entry)`**: melakukan linking dan menghasilkan keluaran akhir.
- **`compile(src, filename)`**: menjalankan seluruh pipeline dari source hingga byte modul.
- **`build_file(path)`**: membangun program dari berkas dan menulis hasilnya ke disk.

## Penggunaan

```rust
use brakwm_easy::BrakWm;

let pipeline = BrakWm::new().with_optimize(true);
let bytes = pipeline.compile("fn main() -> i32 { 42 }", "inline.brk")?;
```

## Catatan

Crate ini praktis untuk pengujian cepat dan demonstrasi, tetapi tidak menggantikan pipeline `brakwm-tool` yang lengkap. Konfigurasi pipeline di sini sengaja dibuat minimal agar mudah dipahami.
