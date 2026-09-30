# brakwm-opt-traits

Trait dan manajer pipeline untuk pass optimasi LIR.

## Trait

- **`LirOptimizationPass`**: trait yang harus diimplementasikan setiap pass. Metode `name()` mengembalikan nama pass untuk laporan, dan `run(program)` menjalankan transformasi terhadap `LirProgram` dan mengembalikan hasilnya.

## PassManager

`PassManager` menjalankan kumpulan pass berurutan atas satu program LIR.

- **`add_pass(pass)`**: menambahkan pass ke dalam pipeline.
- **`with_iterations(n)`**: menetapkan berapa kali seluruh pipeline diulang.
- **`with_verbose(on)`**: mengaktifkan pencetakan setiap pass yang dijalankan.
- **`load_external_pass(path)`**: memuat pass dari pustaka dinamis.
- **`run(program)`**: menjalankan pipeline dan mengembalikan LIR hasil akhir.

## Penggunaan

```rust
use brakwm_opt_traits::PassManager;
use brakwm_opt_dce::DeadCodeElimination;
use brakwm_opt_cp::CopyPropagation;

let mut passes = PassManager::default();
passes.add_pass(Box::new(DeadCodeElimination));
passes.add_pass(Box::new(CopyPropagation));
let optimized = passes.run(lir)?;
```

## Catatan

Karena setiap pass dibungkus sebagai trait object, urutan penambahan pass menentukan hasil akhir. Pipeline default pada CLI menyusun pass dari yang paling struktural ke yang paling lokal.
