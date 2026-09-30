# BrakWM: Brak Toolkit untuk WebAssembly

BrakWM adalah toolkit modular untuk membangun bahasa pemrograman dari nol hingga menjadi modul WebAssembly, tanpa ketergantungan pada tool eksternal seperti LLVM, GCC, atau MSVC. Targetnya adalah modul `.wasm` yang dijalankan melalui WASI preview 1 di Node.js.

Struktur crate, pembagian tanggung jawab, dan alur pipeline mengikuti proyek Brak asli. Perbedaannya ada pada tiga crate backend, yang disesuaikan agar menghasilkan WASM sebagai ganti executable native:

| Brak (asli) | BrakWM (tujuan WASM) |
| --- | --- |
| `brak-codegen-asm` | `brakwm-codegen-wat` |
| `brak-codegen-obj` | `brakwm-codegen-wasm` |
| `brak-link-native` | `brakwm-link-wasm` |

## Arsitektur Proyek

Proyek ini dibagi menjadi 27 crate (ditambah manifest root) yang masing-masing menangani satu tahap spesifik dalam pipeline kompilasi.

### 1. Inti & Frontend
- **brakwm-core**: Tipe dasar, penanganan error, dan pemetaan kode sumber.
- **brakwm-frontend**: Lexer dan Parser untuk mengubah kode sumber `.brk` menjadi AST.

### 2. Representasi Internal (IR)
- **brakwm-ir-ast**: Abstract Syntax Tree, representasi langsung dari kode.
- **brakwm-ir-hir**: High-level IR; menambahkan tipe pada ekspresi dan item.
- **brakwm-ir-mir**: Mid-level IR dengan Control Flow Graph (CFG) dan variabel lokal.
- **brakwm-ir-lir**: Low-level IR berbasis register virtual, siap untuk optimasi dan codegen.

### 3. Optimasi (`brakwm-opt-*`)
Berbagai modul untuk membuat kode lebih cepat dan kecil:
- **dce**: Menghapus instruksi mati.
- **cp**: Copy propagation dan konstanta.
- **gvn**: Menghapus perhitungan redundan (global value numbering).
- **fold**: Menyederhanakan konstanta dan operasi.
- **inline**: Memasukkan isi fungsi kecil ke pemanggilnya.
- **licm**: Mengeluarkan kode invariant dari loop.
- **jt**: Jump threading untuk menghilangkan lompatan yang tidak perlu.
- **tco**: Tail-call optimization untuk menghindari stack overflow pada rekursi.

### 4. Codegen & Linker
- **brakwm-codegen-wat**: Mengubah LIR menjadi teks WebAssembly Assembly (WAT).
- **brakwm-codegen-wasm**: Mengubah LIR langsung menjadi bytecode `.wasm` biner.
- **brakwm-link-wasm**: Memvalidasi modul dan memastikan entry point tersedia.

### 5. Tooling & Utilitas
- **brakwm-tool**: CLI `brakwm` dengan subcommand `build`, `run`, `emit-ir`, dan `link`.
- **brakwm-codegen-traits**: Trait `CodegenBackend` yang dipakai kedua backend.
- **brakwm-link-traits**: Trait `LinkerBackend` dan tipe object bersama.
- **brakwm-link-archive**: Pembacaan arsip `.a`/`.lib`.
- **brakwm-bitcode**: Serialisasi dan cache IR dalam bentuk JSON.
- **brakwm-test**: Harness pengujian untuk program sumber.
- **brakwm-easy**: Pipeline kompilasi sederhana berpreset.
- **brakwm-lang-lit**: Backend bahasa alternatif "Lit".
- **brakwm-opt-traits**: Trait pass optimasi dan `PassManager`.
- **brakwm-opt-utils**: Helper CFG bersama untuk pass optimasi.

## Cara Menggunakan

### 1. Instalasi
Pastikan Anda memiliki [Rust](https://rustup.rs/) dan [Node.js](https://nodejs.org/) terinstal. Build toolkit:

```bash
cargo build --release
```

### 2. Kompilasi Program
Gunakan **brakwm** untuk mengompilasi file `.brk` menjadi modul `.wasm`:

```bash
# Build modul
./target/release/brakwm.exe build samples/hello.brk -o hello.wasm

# Jalankan langsung lewat harness WASI Node
./target/release/brakwm.exe run samples/hello.brk
```

### 3. Menjalankan Program
Perintah `run` memakai Node.js `node:wasi` dengan opsi `returnOnExit: true`, lalu mengembalikan exit code program ke status proses:

```bash
brakwm run samples/hello.brk      # keluar dengan kode 42
brakwm run samples/full.brk       # keluar dengan kode 15
brakwm run samples/recursion.brk  # keluar dengan kode 55
```

Modul yang dihasilkan mengekspor `_start` (entry point WASI) dan mengimpor `wasi_snapshot_preview1.proc_exit`. Fungsi `main` tetap diekspor agar dapat dipanggil langsung saat pengujian.

### 4. Alur Kerja Pengembangan yang Aman
- **Gunakan Tipe Eksplisit**: BrakWM ketat terhadap tipe data. Selalu definisikan tipe pada `let` dan `fn`.
- **Cek Representasi Internal**: Untuk debug, cetak IR pada level yang relevan:
  ```bash
  brakwm emit-ir samples/recursion.brk ast   # Bentuk parse
  brakwm emit-ir samples/recursion.brk hir   # Tipe
  brakwm emit-ir samples/recursion.brk mir   # CFG
  brakwm emit-ir samples/recursion.brk lir   # Register virtual
  brakwm emit-ir samples/recursion.brk wat   # Teks WebAssembly
  ```
- **Verifikasi dengan Testing**: Jalankan test suite setiap kali kompilator diubah:
  ```bash
  cargo test --workspace
  ```

## Model Eksekusi WASM

Karena WebAssembly tidak memiliki model stack call native, lowering LIR memakai strategi dispatch berbasis program counter:

- Setiap fungsi memakai variabel lokal `$__pc` yang menyimpan indeks blok saat ini.
- Blok CFG dikodekan sebagai `block` bersarang di dalam satu `loop`.
- `br_table` berpindah ke blok yang sesuai dengan `$__pc`.
- Pointer stack global `$__brakwm_sp` mengelola frame stack untuk `alloca` dan akses field struct.
- Layout memori: data mulai di `0x400`, stack di `0x4000`, dengan minimum 2 halaman (128 KiB).

## Status Proyek
- **Validasi**: Modul hello, loop, dan rekursi menghasilkan 42, 15, dan 55 melalui jalur WASI nyata.
- **Zero-Dependency**: Tidak butuh LLVM/GCC; hanya Rust dan Node.js untuk menjalankan.
- **Linker**: Saat ini menerima satu object yang sudah menjadi modul lengkap; penggabungan multi-object belum didukung.
- **Optimasi**: Pipeline pass tersedia; beberapa pass belum diuji secara depth pada program besar.

---

*Dibuat untuk memastikan modularitas, kejujuran performa, dan kemudahan pengembangan bahasa.*
