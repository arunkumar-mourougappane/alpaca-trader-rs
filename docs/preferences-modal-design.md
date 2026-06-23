# Preferences Modal — Design & Implementation Research

Research notes for issue #147 follow-on work: an in-app `Preferences` modal
that lets users edit `config.toml` settings without leaving the TUI.

---

## Context

`AppPrefs` is loaded from `config.toml` at startup and accessed via
`App::prefs`. Currently there is no in-app editing UI — users must exit,
hand-edit the TOML file, and restart. This doc captures what needs to be
built and how it fits the existing architecture.

---

## Existing Architecture (reference)

### Modal lifecycle

```
User presses hotkey
  → handle_key() (src/update.rs)
      → app.modal = Some(Modal::Preferences(PrefsState::new(&app.prefs)))
  → render() sees modal → modals::render() dispatches to render_preferences()
  → handle_modal_key() (src/input/modal.rs) routes keys to preferences handler
  → Ctrl-S  → app.prefs = state.draft; write_to(config_path); app.modal = None
  → Esc     → (confirm if dirty) app.modal = None
```

### Files to touch

| File | Change |
|---|---|
| `src/app.rs` | Add `Modal::Preferences(PrefsState)` variant; add `PrefsState`, `PrefsSection`, `DropdownState` structs/enums |
| `src/ui/modals.rs` | Add `render_preferences()` function; wire into `render()` dispatch |
| `src/input/modal.rs` | Add `Modal::Preferences` arm to `handle_modal_key()` |
| `src/update.rs` | Map `P` keypress → `app.modal = Some(Modal::Preferences(...))` |
| `src/ui/ui-mockups.md` | ✅ Already updated with ASCII mockups |

---

## Proposed State Structs

```rust
// src/app.rs

#[derive(Debug, Clone, PartialEq)]
pub enum PrefsSection {
    App,
    Ui,
    Stream,
    Notifications,
    Safety,
    Proxy,
}

impl PrefsSection {
    pub const ALL: &'static [PrefsSection] = &[
        Self::App, Self::Ui, Self::Stream,
        Self::Notifications, Self::Safety, Self::Proxy,
    ];

    pub fn label(&self) -> &'static str {
        match self {
            Self::App           => "App",
            Self::Ui            => "UI",
            Self::Stream        => "Stream",
            Self::Notifications => "Notifications",
            Self::Safety        => "Safety",
            Self::Proxy         => "Proxy",
        }
    }

    pub fn field_count(&self) -> usize {
        match self {
            Self::App           => 2,   // default_env, refresh_interval_ms
            Self::Ui            => 7,   // theme, 4x show_*, default_equity_range, chart_marker
            Self::Stream        => 2,   // reconnect_max_attempts, reconnect_backoff_base_ms
            Self::Notifications => 3,   // fill_enabled, fill_ttl, status_ttl
            Self::Safety        => 1,   // confirm_watchlist_remove
            Self::Proxy         => 3,   // http, socks5, no_proxy
        }
    }
}

#[derive(Debug, Clone)]
pub struct DropdownState {
    pub options: Vec<String>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub struct PrefsState {
    pub draft: AppPrefs,
    pub section: PrefsSection,
    pub field_index: usize,
    pub dropdown: Option<DropdownState>,
    pub editing_buf: Option<String>,  // buffer for in-place numeric edit
    pub dirty: bool,
}

impl PrefsState {
    pub fn new(current: &AppPrefs) -> Self {
        Self {
            draft: current.clone(),
            section: PrefsSection::App,
            field_index: 0,
            dropdown: None,
            editing_buf: None,
            dirty: false,
        }
    }
}
```

---

## Field Definitions per Section

Each field needs to know its label, type, and how to read/write from `AppPrefs`.

### Field type taxonomy

| Type | Widget | Keys |
|---|---|---|
| `Bool` | `[✓]` / `[ ]` checkbox | `Enter`/`Space` toggles |
| `Enum(options)` | `[ value ▾ ]` dropdown | `Enter` opens; `↑↓` selects; `Enter` confirms |
| `U64` / `U32` | `[ 5000  ]` text box | `Enter` enters edit mode; digits + `Backspace`; `Enter` confirms |
| `OptString` | `[ value  ]` or `[ —     ]` | `Enter` enters edit mode; any char; `Backspace`; `Enter` confirms; `Delete` clears to `None` |

### App section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `default_env` | Enum | `"live"`, `"paper"` |
| 1 | `refresh_interval_ms` | U64 | — |

### UI section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `theme` | Enum | `"default"`, `"dark"`, `"high-contrast"` |
| 1 | `show_account_panel` | Bool | — |
| 2 | `show_watchlist` | Bool | — |
| 3 | `show_positions` | Bool | — |
| 4 | `show_orders` | Bool | — |
| 5 | `default_equity_range` | Enum | `"1D"`, `"1W"`, `"1M"`, `"YTD"` |
| 6 | `chart_marker` | Enum | `"braille"`, `"dot"`, `"block"`, `"bar"`, `"half_block"` |

### Stream section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `reconnect_max_attempts` | U32 | — |
| 1 | `reconnect_backoff_base_ms` | U64 | — |

### Notifications section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `fill_notifications_enabled` | Bool | — |
| 1 | `fill_notification_ttl_ms` | U64 | — |
| 2 | `status_message_ttl_ms` | U64 | — |

### Safety section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `confirm_watchlist_remove` | Bool | — |

### Proxy section fields

| Index | Label | Type | Options |
|---|---|---|---|
| 0 | `http` | OptString | — |
| 1 | `socks5` | OptString | — |
| 2 | `no_proxy` | OptString | — |

---

## Rendering Layout

```
popup_area(area, 75, 80)   →  outer popup rect
  ├── Block (double border, title " Preferences " / " Preferences ● ")
  └── inner rect split horizontally:
        left  22 cols  →  section sidebar  (Block + List)
        right remaining  →  field pane  (Block + custom field rows)
```

Field rows use `Constraint::Length(1)` per row with a two-column split:
- Left 28 cols: label (dimmed)
- Right remaining: value widget (highlighted when focused)

Dropdown overlays render as a small `Block` popup anchored below the focused
field row, using `frame.render_widget(Clear, dropdown_rect)` first.

---

## Input Handling Outline

```rust
// src/input/modal.rs  (new arm)

Modal::Preferences(state) => {
    if state.dropdown.is_some() {
        handle_prefs_dropdown(state, key);
    } else if state.editing_buf.is_some() {
        handle_prefs_text_edit(state, key);
    } else {
        match key.code {
            KeyCode::Tab | KeyCode::BackTab  => state.cycle_section(key),
            KeyCode::Up | KeyCode::Char('k') => state.field_index = state.field_index.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                let max = state.section.field_count().saturating_sub(1);
                state.field_index = (state.field_index + 1).min(max);
            }
            KeyCode::Enter | KeyCode::Char(' ') => state.activate_field(),
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                save_prefs(app, state);
                app.modal = None;
            }
            KeyCode::Esc => handle_prefs_esc(app, state),
            _ => {}
        }
    }
}
```

---

## Save & Apply Behaviour

- **Bool, Enum fields** — changes apply to `state.draft` immediately on toggle/select.
- **Numeric / string fields** — changes apply to `state.draft` on `Enter` confirm.
- **`dirty` flag** — set to `true` whenever `state.draft != original`.
- **Ctrl-S** → `app.prefs = state.draft.clone()` then `app.prefs.write_to(path)`.
- **Live theme application** — when `theme` field changes and is confirmed, also
  call `app.current_theme = Theme::from_str(&state.draft.ui.theme)` so the
  preview is immediate (same as the existing `T` key behaviour).
- **Esc with dirty state** → show a small "Discard unsaved changes? (y/N)"
  `Confirm` modal using the existing `Modal::Confirm` infrastructure.

---

## Keybinding

`P` (uppercase) is currently unbound. It is a natural mnemonic for
**P**references and does not conflict with any existing global or
panel-specific binding.

Add to `src/update.rs` in the global key handler block:

```rust
KeyCode::Char('P') => {
    app.modal = Some(Modal::Preferences(PrefsState::new(&app.prefs)));
}
```

Update the Help modal and status bar hint accordingly.

---

## Open Questions / Decisions Needed

1. **Live-apply vs save-only** — Should non-theme fields (e.g. `chart_marker`,
   `show_watchlist`) take effect immediately in the draft view or only after
   save? Immediate is nicer UX but complicates rollback on Esc.

2. **Proxy fields** — These are `Option<String>`. A `Delete` key could clear to
   `None`; or show `[ — ]` placeholder and treat empty string as `None` on
   save. Which is clearer?

3. **Restart-required fields** — `refresh_interval_ms`, `reconnect_*`, and
   `default_env` only take effect on next app start (they control async task
   timings set at spawn time). Should the modal show a `⚠ requires restart`
   hint next to those fields?

4. **Scrolling** — The Proxy and UI sections fit on screen at 80×24. If the
   terminal is shorter, fields need to scroll. A `scroll_offset: usize` in
   `PrefsState` handles this but adds render complexity. Minimum terminal
   height assumption is 24 rows.

5. **Mouse support** — Section sidebar items and field rows should be
   clickable (consistent with other modals). Hit areas stored in
   `app.hit_areas` per frame.
