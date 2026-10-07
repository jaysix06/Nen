# 念 | Nen

念 · Nen · thought, attention, remembrance.

A local-first Windows notes app built with Rust, GPUI and SQLite.

Write it. Keep it accessible. Get reminded. Move on.

## Features

- Debounced background autosave, local SQLite storage and recoverable archive.
- Compact tabs with reorder, middle-click close, reopen and session restoration.
- A compact category picker for Personal, Work, Ideas, pinned notes, reminders
  and archive above the note list.
- Plain text and Markdown writing, undo/redo, find and a formatted reading view.
- Bulleted and numbered lists with automatic continuation and indentation.
- Clickable checklists without extra bullets in editing and Reading view.
  Focused boxes also respond to Space/Enter.
- Body zoom with Ctrl+mouse wheel and footer controls, from 50% to 300%.
  Text and images scale together; note titles keep their original size.
- Paste images with Ctrl+V or insert local PNG, JPEG, WebP and BMP files.
  Nen saves independent copies and previews them in Reading view.
- Pinned notes and indexed local search, with title matches ranked first.
- Note reminders with quick presets, daily/weekly/monthly/custom-day recurrence,
  snooze, completion and native Windows notification actions.
- One floating quick-access window that expands between search, results,
  editing and inline reminders, sharing the main app's notes.

The editor stores Markdown as text. Inserted images use local managed references;
remote images are not loaded by the reading view. Links open in the default
application only when clicked. Frosted surfaces
use a cached, locally blurred background; they do not continuously blur other
desktop windows.
Dim controls the background tint directly. Dark foreground surfaces preserve
text contrast on custom images and solid colors without limiting the slider.
Background transformations are cached between launches.

## Keyboard defaults

| Action | Shortcut |
|---|---|
| New note | Ctrl+N |
| Close / reopen tab | Ctrl+W / Ctrl+Shift+T |
| Next / previous tab | Ctrl+Tab / Ctrl+Shift+Tab |
| Select tab | Ctrl+1 through Ctrl+9 |
| Select category | Ctrl+Shift+1 through Ctrl+Shift+9 |
| Find in note | Ctrl+F |
| Search notes | Ctrl+Shift+F |
| Toggle notes sidebar | Ctrl+B |
| Add reminder | Ctrl+R |
| Pin / archive | Ctrl+Shift+P / Ctrl+Shift+A |
| Settings | Ctrl+, |
| Toggle floating bar, globally | Ctrl+Shift+Space |
| Quick note, globally | Ctrl+Alt+Space |
| Open full app, globally | Ctrl+Alt+N |

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
cargo test --locked --features ui-testing
cargo clippy --locked --features ui-testing --all-targets -- -D warnings
```

If this checkout has portable tools in `.tools/msvc`, the wrapper configures
the compiler, SDK and GPUI shader compiler for the current process:

```powershell
./scripts/cargo.ps1 run
./scripts/cargo.ps1 test --locked --features ui-testing
./scripts/package.ps1
./scripts/profile.ps1
./scripts/cargo.ps1 run --example profile_storage
```

Release builds compile GPUI's shaders with `fxc.exe`. The wrapper sets
`GPUI_FXC_PATH` for the portable SDK. Ordinary developer shells can find
the same tool on PATH or through the Windows SDK installation.

## Local data

Application data is stored under `%LOCALAPPDATA%\Nen\Nen\data`:
`notes.sqlite`, its SQLite journal files, managed backgrounds, the `note-images`
folder and bounded logs. Include `note-images` when backing up notes with images.
The app registers its own Windows notification identity and COM activator under
the current user's registry. Startup registration is optional. It does not
change Windows notification preferences.

Windows must allow notifications for reminder banners to appear. If delivery is
disabled, Nen shows an error and keeps the reminder in the Reminders page.
Quitting stops scheduling until the app opens again. It cannot wake a powered-off
computer. Notes and images stay local. Logs do not include note content.
There are no accounts, telemetry, analytics or advertising. Update checks use
GitHub's public release API over HTTPS; notes and settings are never uploaded.
Writing, search and reminders continue to work without an internet connection.

## Release updates

Nen checks [jaysix06/Nen releases](https://github.com/jaysix06/Nen/releases).
The GitHub repository URL is stored in `Cargo.toml`'s `package.repository`.
A build-time `NEN_UPDATE_REPOSITORY` URL can override it. Builds with
neither configured skip update checks. The application checks once per launch,
including tray launches, and quietly skips offline or failed checks.

Publish stable GitHub releases with semantic-version tags such as `v0.2.8`
and attach the Windows x64 executable as **Nen.exe**. Drafts, prereleases,
older versions and releases without that asset do not show a banner.
The asset must have GitHub's SHA-256 digest. Nen verifies the download's size,
checksum and executable architecture before installing it.

Maintainers can run `./scripts/cargo.ps1 run --locked --example check_updates`
to check the configured source without downloading or installing an update.

Nen must be in a writable folder for automatic installation. A copy of the
current executable runs a hidden update helper, waits for Nen to finish saving
and exit, replaces the executable, and reopens the app with the same local data.
If replacement or relaunch fails, it restores the previous executable and
tries to reopen that version. No administrator prompt is requested.

`NEN_DATA_DIR` can point to a separate directory for development and tests.
The previous `STILL_DATA_DIR` override is also accepted.
Do not set it to a real user's database during automated testing.

The wrapper requests a graceful, save-flushing exit from this checkout's running
development executable before rebuilding. Release packaging excludes UI test
and screenshot support. Windows integration is isolated in `src/platform`;
the current executable targets Windows.

See [architecture](docs/architecture.md) and [validation](docs/validation.md)
for implementation boundaries, measured behavior and remaining validation limits.
