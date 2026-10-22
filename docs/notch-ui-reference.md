# Notch 3.2.1 — referensi UI untuk Cikung

Diperiksa 22 Oktober 2026 dari aplikasi lokal yang ditunjuk pengguna: `C:\Users\pet\AppData\Local\Notch\notch.exe`. SHA-256 `c2422d06cb1f4036371cab299368f51731eafb30356e865d724898528ad90026`, PE x64, 14,658,560 byte; metadata versi 3.2.1. Pemeriksaan ini mempelajari desain; tidak mengubah aplikasi Cikung.

## Yang terlihat langsung

Pengamatan jendela Notch yang sudah berjalan menggunakan Windows Computer Use, dipilih berdasarkan executable persis, memperlihatkan:

- Notch hitam di tengah atas dengan bahu melengkung dan header status kecil.
- Control Center berisi kelompok tombol pintas, tile status, dan slider berbentuk kapsul lebar. Angka berada di dalam slider, bukan kolom terpisah.
- Gear membuka Settings di dalam shell yang sama. Header, tombol kembali/home dan indikator halaman menjaga orientasi.
- Settings mengelompokkan General, Appearance dan Modules. Baris memakai badge ikon berwarna, judul singkat, keterangan sekunder, switch atau pilihan segmented di kanan.
- Pilihan animasi bernama Snappy, Smooth dan Bouncy tersedia. Kehadiran kontrol ini tidak membuktikan preset yang aktif, FPS, atau kurva animasi aktual.

Tree aksesibilitas mengidentifikasi `Tauri + React + Typescript` dan dokumen `http://tauri.localhost/`. Ini cocok dengan aset frontend tertanam yang diperiksa secara statis. Aplikasi ini bukan bukti bahwa Cikung perlu mengganti framework.

Catatan pengamatan: [runtime-observations.json](../reference/notch-3.2.1/runtime-observations.json) dan [settings-tree.txt](../reference/notch-3.2.1/settings-tree.txt). Keduanya adalah pengamatan Computer Use biasa, bukan REA Evidence. Screenshot host transparan juga memperlihatkan aplikasi latar; screenshot tersebut tidak disimpan ke proyek.

Ringkasan statis lengkap: [static-summary.md](../reference/notch-3.2.1/static-summary.md).

## Detail statis yang menjelaskan desain

Aset CSS dan JavaScript frontend yang tertanam berhasil didekode tanpa mengeksekusi kode target. Identitas dan lokasi byte asal dicatat di [shell-asset-extraction.json](../reference/notch-3.2.1/shell-asset-extraction.json); potongan terpilih di [shell-selected-ui-snippets.json](../reference/notch-3.2.1/shell-selected-ui-snippets.json).

- **Shell dan isi memakai animasi berbeda.** Shell mendeklarasikan spring standar stiffness340/damping32/mass0.72; Snappy440/35/0.65; Bouncy300/22/0.75. Parameter ini milik implementasi Notch; tidak setara langsung dengan frekuensi spring Cikung.
- **Perpindahan halaman terarah.** Isi masuk dari x±16px selama200ms; opacity masuk180ms. Keluar memakai120ms untuk posisi dan100ms untuk opacity, dengan child stagger50ms dan delay30ms.
- **Ukuran mengikuti isi.** Lebar panel normal440px, compact400px, large485px. Media dengan lirik normal680px; tinggi media normal285px dan Settings520px. Ini nilai deklarasi, bukan ukuran fisik screenshot pada setiap DPI/preset.
- **Bentuk shell diperbarui bersama ukuran.** Kode mendeklarasikan pembulatan ke device pixel, ResizeObserver/requestAnimationFrame untuk bentuk SVG, radius bahu14–24px dan bawah12–24px saat membesar. Ini kandidat penjelasan tepi yang konsisten, bukan hasil benchmark.
- **Warna berjenjang.** CSS solid memakai latar hitam, sedangkan glass mendeklarasikan alpha/blur tersendiri. Kontrol berwarna menjadi aksen di atas panel gelap. Cikung tetap bisa memakai hitam solid sesuai preferensi pengguna.

REA PE resources: `ev_4b4b7b714c0fb22ee9efa9dc1ddca5ddea68c076c015ba88151bbda47793e096`; Rust metadata: `ev_ae9c97eac4274ea90f36b3e44b4828e2241d972ffc6daa184722285de673b99d`; analisis JavaScript parent: `ev_0dde3d77595879389febc85351744fe7f9c642a6ecdef9603a7808246c8259bc`; summary view: `ev_c55a319c7ad407b8115db009b6cb959b4bbd26fddbc54bdba904dff0f7bf9b84`. JSON ringkasan dan batas analisis tersimpan di folder riset. Ekstraksi shell adalah bukti turunan biasa; analisis REA pada direktori turunan tidak dengan sendirinya membuktikan keterkaitannya dengan EXE, sehingga hash/range ekstraksi tetap diperlukan.

## Pelajaran yang relevan untuk Cikung

| Area | Cikung saat diperiksa | Arah yang bisa diterapkan |
| --- | --- | --- |
| Kontrol cepat | Sebagian memakai tombol teks kecil | Kelompokkan kontrol utama, ikon konsisten, hit area jelas, keterangan saat hover dan label aksesibilitas |
| Volume notifikasi | Slider standar, nilai dan label di luar track | Gunakan satu kapsul lebar dengan fill dan nilai persentase di tengah; pertahankan kontrol keyboard |
| Pengaturan | Jendela terpisah, kartu dengan padding20 dan radius14 | Pertahankan jendela terpisah sesuai permintaan pengguna; rapikan isi menjadi baris sejajar dan kontrol kanan yang konsisten |
| Konten notch | Header40px, expanded host580px, banyak fitur | Kurangi kepadatan satu layar dengan pengelompokan fitur dan navigasi yang tetap terbaca |
| Animasi | Spring critically damped frekuensi22; transisi isi180ms dengan slide4px | Bandingkan tahap perubahan shell, klip dan kemunculan isi; pastikan label tetap stabil saat ukuran berubah dan perpindahan cepat tidak berkedip |
| Identitas | Kucing dan Plus Jakarta Sans | Tetap gunakan identitas Cikung; ambil prinsip tata letak, bukan ikon, merek atau aset milik Notch. Indikator baterai100% dan pin yang sebelumnya dihapus tidak perlu dikembalikan |

Angka Cikung berasal dari `src/notch.rs`, `src/app.rs`, `src/app/notch_ui.rs`, `src/app/settings_pages.rs` dan `src/app/settings_window.rs`; graph lokal masih menunjuk versi sumber lama, sehingga sumber saat ini dibaca langsung. Arah di kolom kanan adalah usulan desain, bukan perubahan yang sudah diterapkan.

## Batas pemeriksaan

Tidak ada kontrol sistem, slider, startup, privasi atau izin yang diubah. Riwayat clipboard tidak dibuka. Klik awal pada header ditolak hit test karena titik jatuh pada jendela latar; input tidak dikirim ke aplikasi latar. Scroll berikutnya ditolak setelah terdeteksi input pengguna dan tidak diteruskan. Aksesibilitas dapat mempertahankan node halaman tersembunyi setelah panel menutup, sehingga keberadaan node saja tidak dipakai sebagai bukti halaman sedang terlihat.

Kehalusan animasi belum diukur lewat frame time atau rekaman FPS. Deklarasi CSS/JS adalah bukti statis, bukan bukti semua jalur dijalankan atau mencapai frame rate tertentu. Tidak ada porting, build Cikung baru, atau klaim kesetaraan fitur dalam tugas ini. Aset Notch disimpan hanya pada folder riset `reference/` yang diabaikan Git dan tidak dimasukkan dalam paket rilis.
