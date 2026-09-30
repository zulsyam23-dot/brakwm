# Samples

Contoh program sumber BrakWM (`.brk`).

## Menjalankan

```bash
brakwm run samples/hello.brk
brakwm run samples/full.brk
brakwm run samples/recursion.brk
```

Perintah `run` membangun program lalu menjalankannya lewat harness WASI Node.js, dan mengembalikan exit code program sebagai exit code perintah.

## Contoh

- **`hello.brk`**: program paling minimal. `fn main() -> i32 { 42 }` menunjukkan bahwa nilai balik blok ditulis sebagai ekspresi tail, bukan statement. Menghasilkan exit code `42`.
- **`full.brk`**: menggabungkan deklarasi fungsi, ekspresi sebagai nilai balik, `let` immutable, `let mut`, loop `while`, dan ekspresi `if` sebagai nilai. `main` menjumlahkan `10` dengan hasil loop yang berakhir di `5`, sehingga menghasilkan exit code `15`.
- **`recursion.brk`**: rekursi `fib` dengan ekspresi `if` pada posisi tail di kedua cabang. Menguji apakah nilai dari kedua cabang benar-benar dikembalikan. Menghasilkan exit code `55` untuk `fib(10)`.

## Catatan

Berkas `.wasm` tidak disertakan di repositori karena merupakan hasil build. Bangun sendiri bila diperlukan:

```bash
brakwm build samples/recursion.brk -o recursion.wasm
```

Fitur yang tidak ditunjukkan di sini, seperti struct, enum, float, dan multi-object linking, belum memiliki contoh program yang setara.

Untuk menelusuri tahap pipeline suatu contoh, gunakan `brakwm emit-ir`:

```bash
brakwm emit-ir samples/recursion.brk mir
```
