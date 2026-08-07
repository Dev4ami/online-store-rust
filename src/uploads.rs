//! Penyimpanan gambar produk ke disk lokal.
//!
//! Alur: terima bytes mentah unggahan → decode (sekaligus validasi tipe) →
//! resize agar hemat → kompres JPEG → simpan ke `static/uploads/<uuid>.jpg`.
//! Folder itu sudah ter-serve oleh `ServeDir::new("static")` di `main.rs`,
//! jadi path publiknya `"/static/uploads/<uuid>.jpg"`.

use std::io::Cursor;
use std::path::Path;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use uuid::Uuid;

/// Direktori penyimpanan relatif terhadap working dir server.
const UPLOAD_DIR: &str = "static/uploads";
/// Batas ukuran file mentah sebelum diproses (5 MB).
const MAX_BYTES: usize = 5 * 1024 * 1024;
/// Sisi terpanjang maksimum setelah resize (px).
const MAX_DIM: u32 = 1000;
/// Kualitas encode JPEG (0–100).
const JPEG_QUALITY: u8 = 80;

/// Simpan gambar unggahan. Balikin path publik (mis. `/static/uploads/ab..cd.jpg`)
/// atau pesan error ramah untuk ditampilkan ke admin.
pub async fn save_image(bytes: Vec<u8>) -> Result<String, String> {
    if bytes.len() > MAX_BYTES {
        return Err("Ukuran gambar maksimal 5 MB.".into());
    }

    // Decode + resize + encode bersifat CPU-bound → jangan blok runtime async.
    let file_name = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let img = image::load_from_memory(&bytes)
            .map_err(|_| "File harus berupa gambar (JPG/PNG/WebP).".to_string())?;

        // Perkecil bila melebihi batas; `resize` menjaga rasio aspek.
        let resized = if img.width() > MAX_DIM || img.height() > MAX_DIM {
            img.resize(MAX_DIM, MAX_DIM, FilterType::Lanczos3)
        } else {
            img
        };

        // Encode ke JPEG (buang alpha; foto produk tak butuh transparansi).
        let rgb = resized.to_rgb8();
        let mut buf = Cursor::new(Vec::new());
        JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY)
            .encode_image(&rgb)
            .map_err(|e| format!("Gagal memproses gambar: {e}"))?;

        std::fs::create_dir_all(UPLOAD_DIR)
            .map_err(|e| format!("Gagal menyiapkan folder unggahan: {e}"))?;

        let name = format!("{}.jpg", Uuid::new_v4());
        std::fs::write(Path::new(UPLOAD_DIR).join(&name), buf.into_inner())
            .map_err(|e| format!("Gagal menyimpan gambar: {e}"))?;
        Ok(name)
    })
    .await
    .map_err(|e| format!("Tugas pemrosesan gambar gagal: {e}"))??;

    Ok(format!("/static/uploads/{file_name}"))
}

/// Hapus file gambar dari disk (best-effort). Aman dipanggil kapan saja:
/// hanya menyentuh file di dalam `UPLOAD_DIR`, ambil komponen nama file saja
/// (anti path-traversal), dan mengabaikan error (mis. file sudah tak ada).
pub fn delete_image(url: &str) {
    const PREFIX: &str = "/static/uploads/";
    let Some(rest) = url.strip_prefix(PREFIX) else {
        return; // URL eksternal / bukan file lokal kita → jangan sentuh.
    };
    // Ambil hanya nama file terakhir; buang segmen path apa pun.
    let Some(name) = Path::new(rest).file_name() else {
        return;
    };
    let _ = std::fs::remove_file(Path::new(UPLOAD_DIR).join(name));
}
