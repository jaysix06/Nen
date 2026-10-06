# Still

A local-first Windows notes app built with Rust, GPUI and SQLite.

Write it. Keep it accessible. Get reminded. Move on.

## Run

Open `dist/Still.exe`. No account or internet connection is required.
Windows needs the Microsoft Visual C++ x64 runtime (`VCRUNTIME140.dll`),
which is already installed on this development machine.
Closing the window keeps Still in the tray by default. Use **Quit** in the
tray menu to exit, or change **Settings → General → Close to tray**.
Reminders work while the process is running, including in the tray. Missed
reminders are handled when Still opens or Windows resumes.

## Features

- Debounced background autosave, local SQLite storage and recoverable archive.
- Blank drafts are discarded quietly when their tab closes. Notes with a title,
  content, pin or reminder are kept.
- Compact tabs with reorder, middle-click close, reopen and session restoration.
- Integrated native window controls and drag regions, with actual notes in the sidebar.
- A compact category picker for Personal, Work, Ideas, pinned notes, reminders
  and archive above the note list. Create, rename and remove categories;
  removing one keeps its notes.
- Plain text and Markdown writing, undo/redo, find and a formatted reading view.
- Pinned notes and indexed local search, with title matches ranked first.
- Note reminders with quick presets, daily/weekly/monthly/custom-day recurrence,
  snooze, completion and native Windows notification actions.
- One floating quick-access window that expands between search, results,
  editing and inline reminders, sharing the main app's notes.
- Configurable app and global shortcuts, conflict detection and registration rollback.
- Dark by default, a bundled local lake wallpaper, four restrained accent colors,
  Light/Dark/System themes, editor sizing, local image backgrounds, fit, blur,
  dim, saturation and opacity controls, plus Opaque/Frosted/Clear surfaces.
- Tray access, optional startup registration, floating placement and opacity,
  focus-loss hiding, position memory and reduced-motion support.

The editor stores Markdown as text. Remote images are not loaded by the reading
view. Links open in the default application only when clicked. Frosted surfaces
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
| Find in note | Ctrl+F |
| Search notes | Ctrl+Shift+F |
| Toggle notes sidebar | Ctrl+B |
| Add reminder | Ctrl+R |
| Pin / archive | Ctrl+Shift+P / Ctrl+Shift+A |
| Settings | Ctrl+, |
| Toggle floating bar, globally | Ctrl+Shift+Space |
| Quick note, globally | Ctrl+Alt+Space |
| Open full app, globally | Ctrl+Alt+N |

Use Up/Down and Enter in search to open results. Escape backs out through the
floating states, then hides the compact bar. Shortcuts are editable under
**Settings → Shortcuts**.
Ctrl+, opens settings; pressing it again returns to the editor. Preferences use
left-aligned groups and locally embedded Phosphor Fill icons. The editor font
slider includes a live preview and applies to open notes immediately.

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

Application data is stored under `%LOCALAPPDATA%\Still\Still\data`:
`notes.sqlite`, its SQLite journal files, managed backgrounds and bounded logs.
The app registers its own Windows notification identity and COM activator under
the current user's registry. Startup registration is optional. It does not
change Windows notification preferences.

Windows must allow notifications for reminder banners to appear. If delivery is
disabled, Still shows an error and keeps the reminder in the Reminders page.
Quitting stops scheduling until the app opens again. It cannot wake a powered-off
computer. Notes and images stay local. Logs do not include note content.
There are no accounts, telemetry, analytics, advertising or cloud services.

`STILL_DATA_DIR` can point to a separate directory for development and tests.
Do not set it to a real user's database during automated testing.

The wrapper requests a graceful, save-flushing exit from this checkout's running
development executable before rebuilding. Release packaging excludes UI test
and screenshot support. Windows integration is isolated in `src/platform`;
the current executable targets Windows.

See [architecture](docs/architecture.md) and [validation](docs/validation.md)
for implementation boundaries, measured behavior and remaining validation limits.
