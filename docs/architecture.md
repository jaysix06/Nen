# Still

Still is a local Windows notes utility built with Rust, GPUI and SQLite.

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

Search uses FTS5 trigram indexing for queries of at least three characters.
Short queries use a Unicode lowercase substring function. Lists render visible
rows only. Full note content is loaded for open notes; clean closed buffers are
bounded to 32 additional recent notes.

Windows hooks, global shortcuts, tray events and COM notification callbacks
feed shared application state through channels. Notification registration uses
only the current user's registry. Global shortcut replacements roll back to the
old registration if a new combination is unavailable.

## Design

The interface uses Segoe UI, warm neutral surfaces, a muted green accent,
compact square-edged tabs and a narrow navigation rail. Editor text is larger
than interface text. Borders divide functional regions, rather than boxing
every control. Motion is brief and runs only during an interaction.

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
