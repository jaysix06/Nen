# Still

Still is a local Windows notes utility built with Rust, GPUI and SQLite.

## Implementation order

1. Compile and run a native GPUI window on Windows.
2. Verify transactional SQLite storage, migrations and background persistence.
3. Connect notes, tabs, search, pinning and archive to the editor.
4. Add reminders and an event-driven Windows background service.
5. Add the shared quick-access window and keyboard bindings.
6. Add appearance and shortcut settings, then inspect and profile the app.

## Boundaries

- The UI uses GPUI Kit 0.7.1, which pins its compatible GPUI family. No web runtime.
- SQLite connections belong to a worker thread. UI requests use channels.
- Note summaries are loaded independently from open note content.
- Edits appear immediately, with debounced writes and a shutdown barrier.
- Windows integrations live in the platform module.
- Settings and session state live in SQLite alongside notes.
- Application data and managed images live under the Windows local app-data directory.

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
