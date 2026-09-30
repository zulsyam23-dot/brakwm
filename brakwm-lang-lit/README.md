# brakwm-lang-lit

Tool backend untuk bahasa alternatif "Lit" pada toolkit BrakWM.

## Isi Crate

Binary `brakwm-lit` dibangun sebagai target terpisah, bukan library, sehingga tidak dapat dipakai sebagai dependensi dari crate lain. Crate ini bergantung pada pipeline yang sama dengan toolkit utama, mulai dari frontend, seluruh tahap IR, hingga codegen WASM.

## Bahasa Lit

Lit merupakan bahasa yang jauh lebih sederhana daripada Brak, dan dipakai untuk menguji kemampuan pipeline dengan program yang lebih pendek. Sintaksnya berada di luar cakupan README ini; lihat file contoh di direktori `samples` pada repositori utama.

## Catatan

Karena berupa tool mandiri, `brakwm-lang-lit` tidak diintegrasikan sebagai subcommand `brakwm`. Jalankan binary-nya secara terpisah bila diperlukan.
