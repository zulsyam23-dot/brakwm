# brakwm-opt-cp

Pass copy propagation untuk LIR BrakWM. Menyederhanakan rantai salinan langsung antar register.

## Cara Kerja

Pass ini menelusuri setiap `Mov` yang menyalin register tanpa mengubah nilainya, lalu mengganti pembacaan register salinan dengan register sumber. Substitusi dilakukan berulang sampai tidak ada `Mov` salinan yang tersisa, sehingga rantai `a = b; b = c` dapat diselesaikan menjadi satu referensi langsung.

Pass ini juga menyederhanakan konstanta yang disalin berulang ke register yang sama.

## Penggunaan

```rust
use brakwm_opt_cp::CopyPropagation;
use brakwm_opt_traits::PassManager;

let mut passes = PassManager::default();
passes.add_pass(Box::new(CopyPropagation));
let optimized = passes.run(program)?;
```

Nama pass yang dilaporkan adalah `cp`.
