# brakwm-link-archive

Pembacaan arsip object untuk target WASM, mengikuti tata letak arsip `ar` yang lazim.

## API Utama

- **`Archive::new()`**: membuat arsip kosong.
- **`add_object(obj)`**: menambahkan satu `ObjectFile` beserta simbolnya ke arsip.
- **`len()` dan `is_empty()`**: jumlah object yang tersimpan.
- **`symbols()`**: daftar seluruh simbol yang tersedia di arsip.
- **`to_bytes()`**: menserialisasikan arsip ke bytes.
- **`from_bytes(data)`**: membaca kembali arsip dari bytes.
- **`link_with(linker, entry, base_addr)`**: pintasan yang menjalankan linking dan meneruskan hasilnya.

## Format

Arsip memakai magic `!<arch>\n` dan setiap anggota diawali header tetap yang memuat nama object, waktu, ukuran, dan pengenal owner. Data anggota disimpan apa adanya sehingga `from_bytes` dapat memulihkan object tanpa kehilangan byte modul WASM.

## Penggunaan

```rust
use brakwm_link_archive::Archive;

let mut archive = Archive::new();
archive.add_object(obj);
let bytes = archive.to_bytes()?;
let restored = Archive::from_bytes(&bytes)?;
let out = restored.link_with(&WasmLinker::new(), "main", 0)?;
```

## Catatan

Pembacaan arsip masih menyalin data anggota satu per satu. Format ini sudah benar untuk arsip yang dihasilkan sendiri, tetapi belum dioptimalkan untuk arsip berukuran besar.
