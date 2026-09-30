# brakwm-core

Tipe dasar bersama yang digunakan seluruh crate BrakWM.

## Isi Crate

- **Source & Lokasi**: `SourceLoc`, `Span`, dan `SourceMap` merepresentasikan rentang serta lokasi sumber. `DUMMY_SPAN` dipakai untuk instruksi yang tidak berasal dari ekspresi pengguna.
- **Diagnostic**: `Diagnostic`, `Diagnostics`, dan `Severity` untuk mengumpulkan dan melaporkan pesan. `Diagnostic::error` dan `Diagnostic::warning` adalah pembantu constructing yang umum dipakai.
- **Error**: `Result<T>` adalah alias `Result` dengan error dinamis (`Box<dyn Error>`), sehingga lapisan pipeline tidak perlu menentukan enum error sendiri.
- **Hashing**: trait `ContentHash` beserta `combine_hash` untuk hashing konten IR secara stabil.
- **Versi**: `Version` dan konstanta `BRAKWM_VERSION` menyatakan versi toolkit.

Crate lain mengandalkan `Span` agar format lokasi diagnostic konsisten dari lexer sampai codegen, dan mengandalkan `Result<T>` agar kesalahan dapat diteruskan tanpa konversi berulang.
