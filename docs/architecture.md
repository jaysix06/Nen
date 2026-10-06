# Nen

Nen is a local Windows notes utility built with Rust, GPUI and SQLite.

Nen reuses the legacy data directory when present. The notification
identity, COM activator, database schema and single-instance mutex remain stable
across the rename so existing reminders and data keep working. New installations
use the Nen application-data directory.

The updater checks a configured GitHub repository's latest stable release once
after the main window initializes. HTTP runs on GPUI's background executor with
timeouts, HTTPS-only redirects and bounded metadata/download sizes. The update
banner is immediately above the notes sidebar's Settings button. Version
comparison ignores build metadata and excludes prereleases. Only the repository's
`Nen.exe` release asset is eligible, with a verified SHA-256 digest and x64 PE header.

Downloads and a copy of the trusted current executable are staged in a unique
folder beside Nen. The helper verifies its identity and the parent's image path,
holds the exact parent process handle, signals readiness and waits for process
exit. Nen saves notes, preferences and its session through the storage worker
before committing installation and shutting down. The helper verifies the
download again, keeps the old executable until replacement and relaunch succeed,
and restores it on failure. The reopened app removes only known helper files
inside the verified staging directory. Update checks never send note content.

## Implementation

The shell was compiled on Windows before UI features were added. Storage,
editing, reminders, platform integration and customization were built in
successive compiled and tested slices.

## Boundaries

- The UI uses GPUI Kit 0.7.1, which pins its compatible GPUI family. No web runtime.
- SQLite connections belong to a worker thread. UI requests use channels.
- Note summaries are loaded independently from open note content.
- Edits appear immediately, with debounced writes and a shutdown barrier.
- Windows integrations live in the platform module.
- Settings and session state live in SQLite alongside notes.
- Application data and managed images live under the Windows local app-data directory.

The worker owns one SQLite connection with WAL, full synchronous writes,
foreign keys and transactional schema migrations. Its next-event timeout wakes
for commands, reminder deadlines and Windows clock/resume events; there is no
one-second reminder polling. Recurring reminders retain a series identity so
cancellation removes future occurrences. Monthly reminders retain their day
anchor across shorter months.

Autosave uses a 350 ms debounce. Each buffer has an edited and persisted
revision, so an older acknowledgement cannot mark newer text saved. Failed
writes preserve the dirty buffer. Deletion gates edits and pending saves until
its database acknowledgement, preventing a delayed save from recreating a
deleted note. Shutdown queues dirty buffers and preferences before a database
checkpoint barrier.
The shutdown acknowledgement follows both SQLite handle release and command
channel closure, so subsequent writes fail immediately.

Categories use a versioned migration and a nullable foreign key on notes.
Removing a category clears its associations without deleting notes. Late saves
normalize an association to an already removed category to null, preserving
the latest text. Category loading uses generations to discard stale responses.
The selected category and sidebar visibility are part of the saved session.

Search uses FTS5 trigram indexing for queries of at least three characters.
Short queries use a Unicode lowercase substring function. Lists render visible
rows only. Full note content is loaded for open notes; clean closed buffers are
bounded to 32 additional recent notes.

Windows hooks, global shortcuts, tray events and COM notification callbacks
feed shared application state through channels. Notification registration uses
only the current user's registry. Global shortcut replacements roll back to the
old registration if a new combination is unavailable.

## Design

The interface uses Segoe UI, dark neutral surfaces, a quiet green accent and
compact square-edged tabs. A category popover sits in the notes pane header;
the left pane contains actual notes. GPUI's native window-control hit regions
handle dragging, minimizing, maximizing and closing. Interactive titlebar
controls occlude the parent drag region. The notes pane opens and closes through
a 200 ms interruptible spring, with a configurable Ctrl+B default.

The bundled lake photograph remains local. Image transformations run off the
UI thread and use an atomic disk cache keyed by source metadata and transform
parameters. Dim changes the overlay opacity directly, without image processing
or a minimum tint. Dark foreground surfaces keep custom backgrounds readable.
Contrast checks cover editor text and secondary text across
Light/Dark and Opaque/Frosted/Clear combinations. Editor text is larger than
interface text. Hairline separators divide functional regions; settings use
compact rows. Settled motion requests no frames.

Preferences navigation uses left-aligned icon/label rows grouped into App,
Personalization and Controls. Ctrl+, toggles preferences and restores editor
focus. Font changes invalidate each open editor's text entity, rebuilding
glyphs, wrapping and caret geometry. Phosphor Fill SVG assets are embedded
locally and override the icons used by application and native window controls.
Their source revision and MIT license are included in the package notices.

New blank notes remain in memory until edited or given meaningful metadata.
Closing a blank draft cancels its debounce and queues deletion behind any
already submitted writes. It is removed from reopen history only after storage
acknowledges deletion. Notes with content, a title, a pin, archive status or
reminders retain normal save/close behavior.

The native shell and visual hierarchy were informed by
[Zeron](https://github.com/zeronsh/zeron), without copying its implementation.

The [beUI motion catalog](https://beui.dev/components/motion),
[Morphing Tabs](https://beui.dev/components/blocks/morphing-tabs) and
[Dynamic Island](https://beui.dev/components/blocks/dynamic-island) informed
continuity, drag thresholds and reduced-motion handling. Their browser
implementations are not dependencies of this application.

Quick access is one persistent GPUI window with compact, search, results,
editor and reminder states. Its shell stays anchored at the top while its
width and height change. GPUI springs preserve velocity when the user
changes direction midway through a transition. Settled springs stop
requesting frames.
