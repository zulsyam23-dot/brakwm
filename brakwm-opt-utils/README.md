# brakwm-opt-utils

Helper analisis Control Flow Graph yang dipakai bersama oleh beberapa pass optimasi.

## Fungsi

- **`cfg_successors(func)`**: memetakan indeks blok ke daftar blok yang dapat dicapai langsung darinya, berdasarkan terminator tiap blok.
- **`cfg_predecessors(succ)`**: membalik peta di atas untuk mendapatkan pendahulu tiap blok.
- **`loop_headers(succ, entry)`**: menentukan blok-blok header loop, yaitu simpul yang bisa dicapai dari dirinya sendiri melalui jalur CFG.
- **`dominators(succ, entry)`**: menghitung dominator tiap blok terhadap blok entry.

## Penggunaan

```rust
use brakwm_opt_utils::{cfg_successors, loop_headers};

let succ = cfg_successors(&func);
let headers = loop_headers(&succ, 0);
```

## Catatan

Analisis dilakukan di atas indeks blok bertipe `usize`, bukan objek graf khusus. Pendekatan ini membuat helper ini ringan dan mudah dipakai langsung pada `LirFunction` tanpa alokasi struktur tambahan yang tidak diperlukan.
