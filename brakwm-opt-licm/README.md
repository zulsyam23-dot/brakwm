# brakwm-opt-licm

Pass loop-invariant code motion untuk LIR BrakWM. Mengeluarkan perhitungan yang tidak bergantung pada nilai loop ke bagian sebelum loop.

## Cara Kerja

Pass ini mencari loop melalui header yang bisa dicapai dari dirinya sendiri, kemudian menentukan register mana yang hanya ditulis di dalam loop. Instruksi yang hasilnya tetap sama di setiap iterasi dan hanya membaca nilai yang tidak berubah sepanjang loop dipindahkan ke blok sebelum loop.

Instruksi yang menulis register yang dipakai di dalam loop tidak dapat dipindahkan, karena pemindahannya akan mengubah nilai yang dibaca loop. Pengecekan ini membuat pass aman terhadap loop yang mengubah variabel akumulator.

## Penggunaan

```rust
use brakwm_opt_licm::LoopInvariantCodeMotion;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(LoopInvariantCodeMotion));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `licm`.
