Trên Linux build `.exe` Windows bằng cross-compile được. Với project Rust của ông, cách đơn giản nhất là dùng target **MinGW GNU**.

Trên Ubuntu/Debian:

```bash
sudo apt update
sudo apt install -y mingw-w64 gcc-mingw-w64-x86-64
```

Thêm Windows target:

```bash
rustup target add x86_64-pc-windows-gnu
```

Trong thư mục project:

```bash
cargo build --release --target x86_64-pc-windows-gnu
```

File `.exe` sẽ nằm ở:

```text
target/x86_64-pc-windows-gnu/release/wireless-pa.exe
```

Nếu linker không được Rust tự detect, tạo `.cargo/config.toml`:

```toml
[target.x86_64-pc-windows-gnu]
linker = "x86_64-w64-mingw32-gcc"
ar = "x86_64-w64-mingw32-ar"
```

Rồi:

```bash
cargo clean
cargo build --release --target x86_64-pc-windows-gnu
```

Với stack của app:

```text
egui / eframe
CPAL
DSP Rust
AEC Rust
adaptive resampling
```

thì hướng này hợp lý vì phần audio Windows của CPAL sẽ compile sang WASAPI cho target Windows; app **không chạy ALSA/PipeWire chỉ vì build trên Linux**.

### Nếu muốn `.exe` theo MSVC

Windows Rust thông thường hay dùng:

```text
x86_64-pc-windows-msvc
```

thay vì GNU. Cross-build MSVC từ Linux thì dùng `cargo-xwin` tiện hơn:

```bash
cargo install cargo-xwin
rustup target add x86_64-pc-windows-msvc
```

Sau đó:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc
```

Output:

```text
target/x86_64-pc-windows-msvc/release/wireless-pa.exe
```

Tôi nghiêng về **`cargo-xwin + x86_64-pc-windows-msvc` cho bản release cuối**, còn GNU target rất tiện để test cross-build nhanh.

Nếu app egui mở kèm một cửa sổ terminal đen trên Windows, thêm đầu `src/main.rs`:

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
```

Khi build release:

```bash
cargo xwin build --release --target x86_64-pc-windows-msvc
```

thì double-click sẽ chỉ hiện GUI, không có console window.

