# brakwm-opt-dce

Pass dead code elimination untuk LIR BrakWM. Menghapus instruksi yang hasilnya tidak pernah dibaca.

## Cara Kerja

Pass ini membangun himpunan register yang dibaca pada program, lalu menghapus setiap `Mov`, arithmetic, atau operasi lain yang menulis register yang tidak termasuk himpunan tersebut. Blok yang menjadi kosong setelah penghapusan tidak ikut dibuang, sehingga struktur CFG tetap utuh dan pass lain tidak perlu beradaptasi dengan blok yang hilang.

Operasi yang mungkin memiliki efek samping, seperti `Call`, dipertahankan karena hasilnya dapat digunakan oleh bagian program lain.

## Penggunaan

```rust
use brakwm_opt_dce::DeadCodeElimination;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(DeadCodeElimination));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `dce`.
