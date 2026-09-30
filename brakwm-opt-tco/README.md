# brakwm-opt-tco

Pass tail-call optimization untuk LIR BrakWM. Saat pemanggilan berada pada posisi tail, pass menggantinya dengan pemindahan argumen ke parameter dan lompatan ke blok masuk fungsi.

## Cara Kerja

Pass mengenali dua bentuk: `Call` yang langsung diikuti `Ret`, dan `Call` lalu `Mov hasil` lalu `Ret hasil`. Pada kedua bentuk tersebut, argumen disalin ke parameter fungsi yang dipanggil, lalu kontrol dialihkan ke blok masuk fungsi. Dengan begitu tidak ada frame stack baru yang ditambahkan, sehingga rekursi yang dalam tidak lagi tertahan oleh batas stack.

Bentuk kedua memerlukan penyesuaian tambahan: karena hasil pemanggilan disalin ke register lain sebelum `Ret`, pass harus mempertahankan register hasil tersebut ketika mengalihkan kontrol.

## Batas Pass

Pemanggilan rekursif mutual, pemanggilan tidak langsung, dan pemanggilan yang bukan tail call tidak dioptimalkan. Karena argumen dapat saling tumpang tindih, argumen ditampung lebih dahulu sebelum disalin ke parameter, supayaizalunan tidak menimpa nilai yang masih dibutuhkan.

## Penggunaan

```rust
use brakwm_opt_tco::TailCallOptimization;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(TailCallOptimization));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `tco`.
