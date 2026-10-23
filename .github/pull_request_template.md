## Ringkasan Perubahan
<!-- Jelaskan secara singkat perubahan, fitur, atau perbaikan bug yang dibuat -->

## Tipe Perubahan
- [ ] Bug fix (perbaikan non-breaking)
- [ ] Fitur baru (penambahan fungsi baru)
- [ ] Refactoring / Optimasi performa
- [ ] Dokumentasi / CI

## Checklist Kualitas (Wajib Diisi)
- [ ] Sudah menjalankan `./scripts/smoke_test.sh` dan semua checks lulus (fmt, clippy, tests).
- [ ] **Rule 1**: Tidak ada `println!` yang bocor ke `stdout` pada MCP server/tools (semua log ke `stderr` / `tracing`).
- [ ] **Rule 2**: Setiap field struct parameter MCP di `crates/gdrive-mcp/src/models.rs` memiliki doc-comment (`/// ...`).
- [ ] **Rule 3**: Panggilan Google Drive API baru menyertakan parameter Shared Drives (`supportsAllDrives=true`).
- [ ] **Rule 4**: Tidak ada credential, token, atau secret yang ter-commit.
- [ ] Unit test / integration test sudah ditambahkan atau diperbarui sesuai perubahan.
