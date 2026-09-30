# brakwm-opt-jt

Pass jump threading untuk LIR BrakWM. Menghilangkan lompatan yang melewati blok tanpa keputusan.

## Cara Kerja

Pass ini membangun peta lompatan: setiap `Jmp` yang menuju sebuah blok dicatat, lalu label tujuan tersebut diganti dengan label tujuan akhir bila blok yang dituju hanya berisi lompatan lain. Proses ini diulang sampai tidak ada lagi rantai lompatan.

`Br` dengan dua tujuan tidak disentuh, karena kedua cabangnya merupakan keputusan yang harus tetap dijalankan.

## Penggunaan

```rust
use brakwm_opt_jt::JumpThreading;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(JumpThreading));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `jt`.
