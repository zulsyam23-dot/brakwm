# brakwm-opt-inline

Pass inlining untuk LIR BrakWM. Memasukkan isi fungsi kecil langsung ke dalam pemanggilnya.

## Cara Kerja

Pass ini mengukur badan tiap kandidat fungsi dan hanya membuat inline ketika badannya cukup kecil serta tidak mengandung kontrol aliran yang rumit. Untuk setiap pemanggilan yang lolos penyaringan, argumen disalin ke parameter fungsi, badan fungsi disalin ke blok pemanggil, dan register hasil fungsi dipetakan ke register hasil pemanggilan.

Pemanggilan rekursif langsung dan fungsi yang ukuran badannya melampaui ambang tidak di-inline, karena keduanya cenderung menaikkan ukuran kode tanpa mengurangi overhead pemanggilan secara berarti.

## Penggunaan

```rust
use brakwm_opt_inline::Inlining;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(Inlining));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `inline`.
