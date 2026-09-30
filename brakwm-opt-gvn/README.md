# brakwm-opt-gvn

Pass global value numbering untuk LIR BrakWM. Menghapus perhitungan ekspresi yang hasilnya sudah pernah dihitung.

## Cara Kerja

Pass ini memberi nomor pada setiap ekspresi murni (misalnya `a + b` atau perbandingan) berdasarkan operand dan tipenya. Ketika ekspresi yang sama muncul lagi dengan operand yang belum berubah, hasil yang sudah dihitung sebelumnya dipakai ulang dan operasi kedua dihapus.

Pass hanya berlaku pada operasi yang deterministik dan tidak memiliki efek samping. Operasi bergaya `Call` serta akses memori yang bisa berubah di antara dua titik program tidak digabung.

## Penggunaan

```rust
use brakwm_opt_gvn::GlobalValueNumbering;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(GlobalValueNumbering));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `gvn`.
