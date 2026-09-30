# brakwm-ir-mir

Mid-level IR: HIR yang diturunkan menjadi Control Flow Graph (CFG) eksplisit dengan variabel lokal.

## Struktur Data

- **Program**: `MirProgram` berisi `functions`, `extern_functions`, `structs`, dan `enums`.
- **Fungsi**: `MirFunction` memuat daftar `MirLocal`, `ret_ty`, dan daftar `MirBlock`. Parameter dialokasikan sebagai lokal pertama, lalu temporer menyusul sesuai urutan alokasi.
- **Blok**: `MirBlock` adalah node CFG basic block yang memuat `insts: Vec<MirInst>` dan satu `MirTerminator`.
- **Instruksi & Terminator**: `MirInst` mendeskripsikan operasi (assignment, call, arithmetic), sedangkan `MirTerminator` (`Return`, `Jump`, `Branch`) menandai akhir blok. Tipe `LocalId` dan `BlockId` hanyalah indeks `usize`.
- **Builder**: `MirBuilder` menyimpan state selama lowering (lokal saat ini, blok saat ini, daftar blok). Instruction builder memastikan setiap blok berakhir dengan tepat satu terminator.

## Tail Expression dan Branch Bernilai

Blok yang punya `tail` menurunkan ekspresi akhir ke temporer lalu mengassign-nya ke `dest`. Untuk ekspresi `if`, `lower_if` membuat blok `if_then`, `if_else`, dan `if_after`, lalu menurunkan masing-masing cabang langsung ke `dest` menggunakan tipe tujuan sebenarnya. Ini yang memungkinkan fungsi seperti `fib` mengembalikan nilai dari kedua cabang:

```
fn fib(n: i32) -> i32 {
    if n <= 1 { n } else { fib(n - 1) + fib(n - 2) }
}
```

Menyodzi `dest_ty` yang benar pada setiap cabang penting: jika cabang diturunkan sebagai `Void`, nilai balik kedua cabang akan hilang dan fungsi hanya mengembalikan register yang belum diinisialisasi.

## Penggunaan

```rust
let hir = HirLower::new().lower(ast)?;
let mir = MirLower::new().lower(hir)?;
```

## Catatan

`MirLower` mempertahankan urutan dan identitas lokal sesuai alokasi; ia tidak menyusun ulang atau deduplikasi nama, sehingga `LocalId` yang sudah dipetakan tetap valid.
