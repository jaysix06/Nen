# Changelog

## 0.2.12 - 2026-10-07

### Added

- Ctrl+Shift+R switches between Reading view and editing in the main app and
  floating notes. Change the shortcut in Settings; the Reading view button's
  tooltip shows the current binding.

## 0.2.11 - 2026-10-07

### Fixed

- The floating island now matches the main app's wallpaper, dimming, blur,
  saturation, background opacity, surface style, theme and accent in every state.
- Pill corners use transparent, antialiased rendering instead of a hard-edged
  Windows region. The floating window's opacity control continues to work.

## 0.2.10 - 2026-10-07

This release includes the editor and floating-window updates since 0.2.8.

### Added

- Bulleted, numbered and checklist formatting through the Lists menu, with
  automatic continuation and indentation.
- Body zoom from 50% to 300% using Ctrl+mouse wheel or the footer controls.
- Local image insertion and Ctrl+V image paste, including Windows screenshots.
  Images are stored with notes and displayed in Reading view.

### Fixed

- Checkboxes can be clicked in editing and Reading view, and toggled with the
  keyboard. New checklists no longer include a separate bullet.
- Images grow and shrink with body zoom while preserving their proportions.
- The editor footer aligns with Settings in the sidebar.
- The tab-bar add button stays hidden when no tabs are open.
- Long notes in Reading view keep their tabs, title and footer at stable heights.
- Expanding a floating note into the main app closes the floating window.
- The floating pill has rounded edges without the unwanted window border.

### Changed

- The floating window uses the wallpaper's dominant color with contrasting text
  and icons.

Validation: 49 tests passed; Clippy and formatting checks passed. Native Windows
checks verified checkbox input, bitmap paste, image zoom, footer alignment and
the packaged executable's update/relaunch behavior with notes preserved.
