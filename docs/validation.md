# Validation

Verified on Windows 11 Pro 24H2, x86_64, using Rust 1.99 and the MSVC/Windows SDK
toolchain. The executable uses native GPUI rendering, not a browser.

## Checks

- `cargo test --locked --features ui-testing`: 21 passing tests.
- `cargo clippy --locked --features ui-testing --all-targets -- -D warnings`: clean.
- `cargo fmt -- --check`: clean.
- `cargo build --locked --release`: successful; UI testing features excluded.
- Native main, settings and four floating-state captures were inspected.
  See [main](screenshots/main.png), [compact](screenshots/compact.png),
  [results](screenshots/results.png), [editor](screenshots/note.png),
  [reminder](screenshots/reminder.png) and [appearance](screenshots/settings.png).

Tests cover migrations without data loss, Unicode and literal search, title
ranking, pin/archive/delete, queued-write shutdown flushing, monthly recurrence
anchors, missed recurrence handling, snooze, idempotent completion and series
cancellation. GPUI tests exercise typing, tab closing/reopening, shortcut
capture/conflicts, shared floating editing/reminders, focus restoration,
failed-save preservation and a delete-versus-load race. Image tests verify
that import/transformation leaves the user's original untouched.

Windows tests register real global shortcuts and verify that a failed
replacement restores the old registration. COM notification activation tests
verify argument delivery and application identity checking. Deliberately
breaking the save-failure flag made its regression test fail; the original
implementation was restored and the full suite passed again.

## Measurements

Final release executable: 28,680,192 bytes (27.35 MiB).

| Measurement | Result |
|---|---:|
| New process to native window creation | 695 ms |
| Working set after idle measurement | 67.4 MiB |
| Private memory | 86.4 MiB |
| Visible editor idle CPU, one-core equivalent | 2.19% |
| Hidden/tray idle CPU, one-core equivalent | 0.94% |
| List 5,000 note summaries | 18.02 ms |
| Indexed search in 5,000 notes, mean of 100 | 1.37 ms |

Window profiling used an isolated demo database, the release executable and
separate 10-second visible/hidden samples. These are one machine's measurements,
not cold-boot or input-to-display latency guarantees. CPU is expressed relative
to one logical core, not the whole processor. The visible editor includes caret
redraws; tray idle equates to about 0.08% across this machine's 12 logical cores.
Storage timing used a debug-build, in-memory database with realistic
paragraphs; it excludes disk-write latency and the search UI's 80 ms debounce.

Profiling identified and fixed a poor SQLite join plan: the full-text index now
drives the note join. Native sizing and visibility changes are dispatched
outside GPUI's borrowed update context. Hidden editors release focus and
replaced background assets release their decoded pixels.

## Validation limits

This Windows profile has global toast notifications disabled (`ToastEnabled=0`).
The native notification smoke check correctly returned an unavailable-delivery
error. The OS preference was left unchanged. Visible toast delivery and clicking
Open/Snooze/Done through the Windows notification UI still need verification on
a Windows profile that allows notifications; their native registration,
activation routing and persistence logic are implemented and tested separately.

The desktop automation helper was unavailable, so captures came directly from
GPUI's GPU renderer. Normal, light-theme client areas were visually inspected;
manual checks across high-DPI/multiple-monitor arrangements and screen readers
remain advisable. Actual input-to-display typing latency and cold-boot startup
were not instrumented.

The release is a local executable, without an installer or signing pipeline.
Scratch notes remain until the user archives or deletes them; automatic scratch
expiry is not enabled. Gradient backgrounds and OS desktop acrylic are not
implemented; solid colors and local images provide the background customization.
