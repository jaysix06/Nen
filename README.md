# Still

A native, local-first notes app for Windows. Rust, GPUI and SQLite.

## Build requirements

- Stable Rust with the `x86_64-pc-windows-msvc` target.
- Microsoft C++ Build Tools with the x64 compiler and Windows SDK.
- Windows 10 or 11 with a DirectX 11 capable graphics driver.

GPUI Kit 0.7.1 pins the compatible GPUI crates and supplies native editing,
menus, dialogs and accessibility. The application does not use a browser
or a JavaScript UI runtime.

From a Visual Studio developer shell:

```powershell
cargo run
cargo test
cargo clippy --all-targets -- -D warnings
```

If this checkout has portable tools in `.tools/msvc`, the wrapper configures
the compiler, SDK and GPUI shader compiler for the current process:

```powershell
./scripts/cargo.ps1 run
./scripts/cargo.ps1 test
./scripts/cargo.ps1 build --release
```

Release builds compile GPUI's shaders with `fxc.exe`. The wrapper sets
`GPUI_FXC_PATH` for the portable SDK. Ordinary developer shells can find
the same tool on PATH or through the Windows SDK installation.

## Local data

Notes, reminders, settings and session state belong in the Windows local
application data directory. User notes are never transmitted. No accounts,
telemetry or cloud services are used.

`STILL_DATA_DIR` can point to a separate directory for development and tests.
Do not set it to a real user's database during automated testing.

See [architecture](docs/architecture.md) for the implementation sequence and
the boundaries between the UI, storage and Windows integrations.
