# brakwm-test

Harness pengujian untuk program sumber BrakWM.

## API

- **`SampleExpectation`**: struktur yang mendeskripsikan hasil yang diharapkan, termasuk exit code atau nilai yang dikembalikan.
- **`SampleRunner`**: trait yang diimplementasikan runner eksternal untuk menjalankan program yang sudah dikompilasi ke bentuk tertentu.
- **`TestRunner`**: pembantu yang menerima implementasi `SampleRunner`, lalu menjalankan sample berdasarkan `LirProgram` dan `SampleExpectation` yang diberikan.
- **`constant_exit_lir(code)`**: helper yang menghasilkan `LirProgram` minimal dengan fungsi `main` yang mengembalikan `code`.

## Penggunaan

```rust
use brakwm_test::{TestRunner, constant_exit_lir};

let runner = TestRunner::new(Box::new(wasi_runner));
let ok = runner.run_sample(&constant_exit_lir(42), &expected)?;
```

## Catatan

Crate ini memisahkan logika penentuan harapan hasil dari mekanisme eksekusi, sehingga test sumber dapat berjalan melalui runner WASI (Node.js), maupun lewat mock internal.
