# Validation

Verified on Windows 11 Pro 24H2, x86_64, using Rust 1.99 and the MSVC/Windows SDK
toolchain. The executable uses native GPUI rendering, not a browser.

## Checks

- `cargo test --locked --features ui-testing`: 36 passing tests.
- `cargo clippy --locked --features ui-testing --all-targets -- -D warnings`: clean.
- `cargo fmt -- --check`: clean.
- `cargo build --locked --release`: successful; UI testing features excluded.
- Native dark/light shells, category picker/form, hidden sidebar, preferences
  and four floating-state captures were inspected.
  See [main](screenshots/main.png), [light](screenshots/light.png),
  [categories](screenshots/categories.png), [category form](screenshots/category-form.png),
  [hidden sidebar](screenshots/sidebar-hidden.png), [background](screenshots/background.png),
  [compact](screenshots/compact.png),
  [results](screenshots/results.png), [editor](screenshots/note.png),
  [reminder](screenshots/reminder.png) and [appearance](screenshots/settings.png).
- Normal Windows launches and restored sessions were checked for callback errors.
- Packaged Nen 0.2.7 starts and flushes a graceful shutdown with a clean release log.

The 0.2.7 icon correction crops the outer transparent padding before resizing,
ignoring near-transparent stray pixels and preserving the tile's aspect ratio
and rounded corners. All seven ICO sizes are checked for excess padding.
The padding regression check failed against the original generated icons.
The packaged Windows icon was extracted and inspected; notification registration
uses the new cropped artwork. The original supplied PNG remains unchanged.

The 0.2.6 rename retains the notification identity, COM activator and legacy
single-instance mutex. Data checks verify existing notes and preferences reopen
from the Still directory and new installations use Nen. Startup checks limit
legacy-entry removal to the app's sibling executable. The supplied transparent
artwork is packaged into seven ICO sizes and matching tray/notification PNGs.

The 0.2.5 asset check verifies Regular artwork for plus, minus, close,
maximize and restore controls, while other icons keep Fill artwork.

The 0.2.4 GPUI check switches every dropdown entry using Ctrl+Shift+1–9,
including a user-created category. It checks an out-of-range number, preserves
an existing conflicting binding on upgrade, captures a replacement shortcut,
reports conflicts, applies the replacement immediately and verifies persistence.
The native Windows capture check dispatches the real keyboard mapper's shifted
digit events through all nine bindings. GPUI's physical key representation is
used for shortcut capture and matching, including shifted symbols.

The 0.2.3 note rows span the full notes pane. GPUI checks verify identical row
edges for short and long titles and selection by clicking near the right edge.
The category trigger, bottom Settings button and preferences navigation also
fill their available width. Checks cover their bounds, right-edge clicks and
full-width dropdown items. Hover and selection use the same full-width targets.

The 0.2.2 GPUI checks exercise the relocated category picker, left-aligned
preferences labels, the actual font slider and rendered line height, repeated
Ctrl+, toggles and typing after focus returns. Native captures compare the
normal editor with [28px text](screenshots/font-large.png). Blank-draft checks
verify no initial SQLite row, queued autosave deletion, no reopen history,
quiet disposal and preservation of title-only/body-only notes after restart.
Asset checks verify all application and window-control icons use local
Phosphor artwork, with the control-symbol exceptions introduced in 0.2.5.

The 0.2.1 Dim fix anchors the tint to the image bounds and removes the hidden
58%/65% minimum. A GPUI regression test clicks the actual slider and verifies
the tint covers the image. Native captures at [0%](screenshots/dim-0.png),
[20%](screenshots/dim-20.png) and [80%](screenshots/dim-80.png) show a custom
wallpaper darkening across the range. Contrast checks cover editor, sidebar and
chrome text at seven Dim values in both themes and all three surface modes.

Tests cover migrations without data loss, Unicode and literal search, title
ranking, pin/archive/delete, queued-write shutdown flushing, monthly recurrence
anchors, missed recurrence handling, snooze, idempotent completion and series
cancellation. GPUI tests exercise typing, tab closing/reopening, shortcut
capture/conflicts, shared floating editing/reminders, focus restoration,
failed-save preservation and a delete-versus-load race. Image tests verify
that import/transformation leaves the user's original untouched and cached
transformations avoid repeated writes. Category tests exercise creation and
rename through real dialogs, cancellation and confirmation of deletion,
category filtering, late autosave after category removal and Ctrl+B persistence.
Legacy preference checks preserve chosen colors/themes. Contrast tests verify
at least 4.5:1 editor/secondary text against extreme custom backgrounds across
both themes and all three surface modes. The dialog/keyboard persistence test
runs with reduced motion enabled; normal motion is inspected in GPU captures.

Windows tests register real global shortcuts and verify that a failed
replacement restores the old registration. COM notification activation tests
verify argument delivery and application identity checking. Deliberately
breaking the save-failure flag made its regression test fail; the original
implementation was restored and the full suite passed again.
Removing the sidebar toggle's state change also caused its regression test to
fail; the original code was restored before the final suite.

## Measurements

Nen 0.2.7 release executable: 32,000,000 bytes (30.52 MiB), including its local wallpaper.
The performance samples below were collected with 0.2.0.

| Measurement | Result |
|---|---:|
| New process to native window creation | 447–779 ms |
| Working set after idle measurement | 80.3–82.8 MiB |
| Private memory | 103.2–104.8 MiB |
| Visible editor idle CPU, one-core equivalent | 2.03–9.06% |
| Hidden/tray idle CPU, one-core equivalent | 0.62–0.86% |
| Visible / hidden CPU, normalized over 12 logical cores | 0.17–0.75% / 0.05–0.07% |
| List 5,000 note summaries | 23.71 ms |
| Indexed search in 5,000 notes, mean of 100 | 1.93 ms |

Window profiling used an isolated demo database, the release executable and
separate 10-second and 20-second visible/hidden samples with the wallpaper loaded.
These are one machine's measurements,
not cold-boot or input-to-display latency guarantees. CPU is expressed relative
to one logical core unless the row explicitly normalizes over all 12 cores.
The visible editor includes caret redraws and varied between the two runs.
Storage timing used a debug-build, in-memory database with realistic
paragraphs; it excludes disk-write latency and the search UI's 80 ms debounce.

Profiling identified and fixed a poor SQLite join plan: the full-text index now
drives the note join. Native sizing and visibility changes are dispatched
outside GPUI's borrowed update context. Hidden editors release focus and
replaced background assets release their decoded pixels.
Windows are first created hidden, then presented after view initialization;
floating activation invalidates sizing after the initial native placement.
Native drag loops run outside the borrowed view, and both the compact icon
and expanded header can move the floating window.

## Validation limits

This Windows profile has global toast notifications disabled (`ToastEnabled=0`).
The native notification smoke check correctly returned an unavailable-delivery
error. The OS preference was left unchanged. Visible toast delivery and clicking
Open/Snooze/Done through the Windows notification UI still need verification on
a Windows profile that allows notifications; their native registration,
activation routing and persistence logic are implemented and tested separately.

The desktop automation helper was unavailable, so captures came directly from
GPUI's GPU renderer. Dark and light client areas were visually inspected;
manual checks across high-DPI/multiple-monitor arrangements and screen readers
remain advisable. Actual input-to-display typing latency and cold-boot startup
were not instrumented.

The release is a local executable, without an installer or signing pipeline.
Scratch notes remain until the user archives or deletes them; automatic scratch
expiry is not enabled. Gradient backgrounds and OS desktop acrylic are not
implemented; solid colors and local images provide the background customization.
