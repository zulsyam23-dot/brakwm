# brakwm-link-traits

Tipe object bersama dan trait backend linker, dipakai oleh `brakwm-link-wasm` maupun `brakwm-link-archive`.

## Tipe

- **`ObjectFile`**: satu unit masukan linker. Untuk target WASM, `data` berisi byte modul yang akan disahkan, sedangkan `symbols` mencantumkan simbol yang diekspor.
- **`LinkerOutput`**: hasil akhir linking, yaitu byte modul akhir beserta formatnya (`wasm`).

## Trait

- **`LinkerBackend`**: trait untuk backend linker. Metode `link(objects, entry, base_addr)` menerima daftar object, nama entry point, dan alamat dasar, lalu mengembalikan `LinkerOutput`. Trait mensyaratkan `Send + Sync` agar dapat dipakai paralel.

## Penggunaan

```rust
use brakwm_link_traits::{LinkerBackend, ObjectFile};

let obj = ObjectFile { name: "main".into(), data: bytes, symbols };
let out = WasmLinker::new().link(&[obj], "main", 0)?;
```

## Catatan

`base_addr` dipertahankan untuk kompatibilitas dengan kontrak linker asli, tetapi backend WASM saat ini tidak memakainya karena modul WASM tidak memerlukan relokasi alamat tetap.
