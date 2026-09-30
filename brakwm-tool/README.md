# brakwm-tool

CLI utama BrakWM. Menyediakan subcommand `build`, `run`, `emit-ir`, dan `link`.

## Perintah

### `brakwm build <INPUT>`

Mengompilasi satu file `.brk` menjadi modul `.wasm`.

- `-o, --output <OUTPUT>`: jalur keluaran, default `<input>.wasm`.
- `-O, --opt <OPT>`: level optimasi `0` sampai `3`, default `0`.

### `brakwm run <INPUT> [ARGS]...`

Build lalu menjalankan program lewat harness WASI Node.js, lalu mengembalikan exit code program sebagai exit code perintah. Argumen tambahan diteruskan ke program.

### `brakwm emit-ir <INPUT> <STAGE>`

Menampilkan IR pada tahap yang dipilih: `ast`, `hir`, `mir`, `lir`, atau `wat`. Perintah ini merupakan alat utama untuk menelusuri bug antar tahap.

### `brakwm link --output <OUTPUT> [INPUTS]...`

Melakukan linking atas object file menjadi modul `.wasm`. Opsi `--entry` menentukan entry point, default `main`.

## Pipeline Build

`build` menjalankan rangkaian berikut:

1. Parse source `.brk` menjadi AST memakai `brakwm-frontend`.
2. Lowering AST ke HIR memakai `brakwm-ir-hir`.
3. Lowering HIR ke MIR memakai `brakwm-ir-mir`.
4. Lowering MIR ke LIR memakai `brakwm-ir-lir`.
5. Menjalankan pipeline pass optimasi bila `-O` lebih besar dari nol.
6. Codegen LIR menjadi bytecode `.wasm` memakai `brakwm-codegen-wasm`.
7. Validasi modul dengan `brakwm-link-wasm`.

## Harness WASI

Perintah `run` menulis modul ke berkas sementara lalu mengeksekusinya dengan skrip Node.js yang memakai `node:wasi` Versi `preview1` dengan opsi `returnOnExit: true`. Opsi tersebut membuat runtime melaporkan exit code hasil `proc_exit` alih-alih keluar sendiri, sehingga `brakwm` dapat meneruskannya sebagai exit code perintah.

## Penggunaan

```bash
brakwm build samples/hello.brk -o hello.wasm
brakwm run samples/recursion.brk
brakwm emit-ir samples/recursion.brk mir
```

## Catatan

`link` saat ini hanya menerima satu object yang sudah menjadi modul lengkap; penggabungan multi-object belum didukung oleh `brakwm-link-wasm`.
