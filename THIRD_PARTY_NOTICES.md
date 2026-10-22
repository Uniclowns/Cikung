# Third-party notices

Cikung uses eframe/egui, image, serde, serde_json, reqwest, tiny_http, getrandom, qrcode, chrono, rfd, keyring, windows-rs, wasapi and hound on Windows, and mpris on Linux, with their transitive dependencies. Exact versions are recorded in Cargo.lock. Applicable upstream licenses continue to apply; the portable package includes collected dependency license files under `licenses/crates`.

The Windows binary uses the Rust standard library and its statically linked runtime. Rust's distributed runtime copyright and license texts accompany the portable package under `licenses`. The executable imports Windows system libraries and does not require separate MinGW runtime DLLs.

CouCou by Louis-CFM was used as a source reference only. Its MIT source license and separate asset restrictions were reviewed. No CouCou or Mochi artwork, icons, audio, branding, or source code is included in Cikung.

The supplied orange-cat illustration was processed using the built-in image generation tool to prepare a transparent sprite and the head-only rendition used in 0.2. No ownership of the original artwork is asserted by this project. The project's MIT license covers original code, not supplied artwork. Prompts and provenance are recorded in `docs/asset-provenance.md`.

## Plus Jakarta Sans

Plus Jakarta Sans is embedded from the official Google Fonts distribution, under the SIL Open Font License 1.1. Copyright and complete license: `assets/fonts/OFL-PlusJakartaSans.txt` (packaged as `licenses/OFL-PlusJakartaSans.txt`). Source: https://github.com/google/fonts/tree/main/ofl/plusjakartasans
