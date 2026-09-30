# brakwm-opt-fold

Pass constant folding untuk LIR BrakWM. Menghitung nilai operasi yang semua operand-nya sudah diketahui pada saat kompilasi.

## Cara Kerja

Pass ini menjalankan propagasi konstanta sederhana di dalam satu basic block. Ketika sebuah instruksi memiliki seluruh operand berupa konstanta, instruksi itu dievaluasi langsung dan diganti dengan satu konstanta hasil. Proses ini diulang sampai tidak ada instruksi lagi yang dapat dilipat.

Operasi yang disentuh mencakup aritmetika integer, perbandingan, dan operasi logika. Pembagian dan sisa pembagian dengan pembagi nol dibiarkan apa adanya, karena hasilnya tidak terdefinisi.

## Penggunaan

```rust
use brakwm_opt_fold::ConstantFolding;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(ConstantFolding));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `fold`.
