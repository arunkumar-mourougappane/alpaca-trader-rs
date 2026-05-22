# Account Management Design

> **Status:** Design draft — v0.1  
> **Scope:** In-app credential entry, multi-account profiles, and a settings modal  
> **Codebase base:** v0.6.0  

---

## Overview

`alpaca-trader-rs` currently resolves credentials before entering raw mode, persists
preferences in `~/.config/alpaca-trader/config.toml`, and has no way to change
credentials or settings without restarting the process. This document specifies three
related features that close that gap:

| Sub-feature | User story |
|---|---|
| **A. In-app Credential Entry** | Enter, update, or clear API keys from inside the TUI without restarting |
| **B. Multi-Profile Support** | Define named account profiles in `config.toml`; switch at runtime |
| **C. Settings Modal** | Edit all `AppPrefs` fields from inside the TUI with immediate persistence |

All three features share the same event/command bus, the same modal rendering
pipeline (`src/ui/modals.rs` → `src/input/modal.rs`), and the same TEA update
loop (`src/update.rs`).

---

## Current Architecture (Brief Reference)

- **Credential resolution** happens entirely in `src/credentials.rs::resolve()`, which
  calls `rpassword` if the keychain is empty. It **must** run before
  `enable_raw_mode()` because it writes to `stderr` and reads from `stdin`.
- **Configuration** lives in `src/config.rs` (`AlpacaConfig` / `AlpacaEnv` /
  `ResolvedCredentials`). After startup `AlpacaConfig` is immutable — it's cloned
  into every background task.
- **Preferences** are loaded via `AppPrefs::load()` at startup and stored on `App`.
  `AppPrefs::write_to()` already exists but is never called at runtime.
- **Modals** are a `Option<Modal>` on `App`; the renderer in `src/ui/modals.rs` is a
  single `match` dispatch; the input handler in `src/input/modal.rs` follows the
  same pattern.
- **Async boundary**: all mutations cross from synchronous `update()` to the async
  world via `command_tx: mpsc::Sender<Command>`. Results come back as `Event`
  variants on the main event channel.
- **Task lifecycle**: five long-running `tokio::spawn` tasks share a single
  `CancellationToken`. There is currently no mechanism to replace a running task
  after startup.

---

## Feature Scope

### A. In-App Credential Management

- First-run setup: detect missing keychain credentials *after* entering raw mode
  and launch `Modal::CredentialEntry` before data polling starts.
- Update credentials for the active profile without restarting.
- Clear/reset credentials for any profile from within the TUI.
- Trigger a full reconnect (cancel old tasks, rebuild `AlpacaClient`, re-spawn tasks)
  after new credentials are confirmed.

### B. Multi-Account / Profile Support

- Named profiles defined in `config.toml` under a `[[profiles]]` array.
- Runtime switching via `Modal::AccountSwitcher`.
- Header badge shows active profile display name and environment.
- Keychain entries namespaced by profile `name` field.
- Backward compatible: a config without `[[profiles]]` behaves exactly as before.

### C. Preferences / Settings Modal

- `Modal::Settings` with per-section navigation (App / UI / Stream / Notifications
  / Safety / Proxy).
- Edits apply immediately to `App::prefs` and are written to disk via
  `AppPrefs::write_to()`.
- Theme changes apply in-frame (same path as the current `T` key) and are now
  also persisted.
- No restart required for any setting except `proxy.*` (noted in the UI).

---

## Data Model

### Profile Schema (`config.toml`)

#### New format (with profiles)

```toml
[app]
default_env        = "live"
refresh_interval_ms = 5000
active_profile     = "personal-live"   # NEW — omit to fall back to legacy behaviour

[ui]
theme              = "default"
# ... unchanged

[[profiles]]
name         = "personal-live"         # machine key, used as keychain namespace
display_name = "Personal Live"         # shown in header badge and switcher
env          = "live"                  # "live" | "paper"

[[profiles]]
name         = "personal-paper"
display_name = "Personal Paper"
env          = "paper"

[[profiles]]
name         = "ira"
display_name = "IRA Account"
env          = "live"
```

#### Rust representation

```rust
// src/prefs.rs — new structs

/// A named account profile. Credentials are stored in the OS keychain,
/// namespaced by `name`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProfileConfig {
    /// Machine key used as the keychain namespace (e.g. `"personal-live"`).
    /// Must be non-empty, contain only ASCII alphanumerics and hyphens.
    pub name: String,
    /// Human-readable label shown in the header badge and account switcher.
    pub display_name: String,
    /// Which Alpaca environment this profile targets.
    pub env: String,   // "live" | "paper"; parsed to AlpacaEnv on use
}

// AppSection gains one new optional field:
pub struct AppSection {
    pub default_env: String,
    pub refresh_interval_ms: u64,
    pub active_profile: Option<String>,   // NEW — None means legacy single-account mode
}

// AppPrefs gains a new optional section:
pub struct AppPrefs {
    pub app: AppSection,
    pub ui: UiSection,
    pub stream: StreamSection,
    pub notifications: NotificationsSection,
    pub safety: SafetySection,
    pub proxy: ProxySection,
    #[serde(default)]
    pub profiles: Vec<ProfileConfig>,     // NEW — empty = legacy mode
}
```

`ProfileConfig::env` is stored as a `String` rather than `AlpacaEnv` to keep
`prefs.rs` free of `config.rs` imports (the `AlpacaEnv` parse happens in
`credentials.rs` where it is already imported).

### Keychain Namespacing

#### Current scheme (unchanged for legacy mode)

| Entry | Service | Username |
|---|---|---|
| Live key    | `alpaca-trader-rs` | `live-api-key` |
| Live secret | `alpaca-trader-rs` | `live-api-secret` |
| Paper key   | `alpaca-trader-rs` | `paper-api-key` |
| Paper secret| `alpaca-trader-rs` | `paper-api-secret` |

#### Profile-namespaced scheme

When `[[profiles]]` are defined, the profile `name` field replaces the
`live`/`paper` prefix:

| Profile `name` | Username (key) | Username (secret) |
|---|---|---|
| `personal-live`  | `personal-live-api-key`  | `personal-live-api-secret`  |
| `personal-paper` | `personal-paper-api-key` | `personal-paper-api-secret` |
| `ira`            | `ira-api-key`            | `ira-api-secret`            |

The service name `"alpaca-trader-rs"` is unchanged across all entries.

The helper in `credentials.rs` is already parameterised by a string prefix
(`kr_prefix`). Extending it to accept a profile name requires no structural
change — only the string passed in changes.

### Backward Compatibility

| `config.toml` state | Behaviour |
|---|---|
| No `[[profiles]]`, no `active_profile` | Existing startup flow; `resolve(env)` called with env from CLI/`default_env`; no profile switcher shown |
| `[[profiles]]` present, `active_profile` set | Profile mode; profile-namespaced keychain lookup; account switcher available |
| `[[profiles]]` present, `active_profile` absent | First profile in array is treated as active; user is nudged to set one explicitly |
| `[[profiles]]` present, named profile has no keychain entry | `CredentialEntry` modal opened automatically for that profile |

No migration step is needed. The existing `[app]`, `[ui]`, `[stream]` etc. sections
are parsed identically. The new `[[profiles]]` table and `active_profile` key default
to absent / `None`, which reproduces legacy behaviour exactly.

---

## UI Design

### Modal — Credential Entry (`Modal::CredentialEntry`)

Handles both first-run setup and in-app credential update.

```
┌─ Credentials — Personal Live ──────────────────────────────┐
│                                                             │
│  Profile:   Personal Live  [LIVE]                           │
│                                                             │
│  Key ID      ┌───────────────────────────────────────┐      │
│  (focused) ▶ │ APCA-API-KEYID-XXXXXXXXXXXXXXXXXXXX   │      │
│              └───────────────────────────────────────┘      │
│                                                             │
│  Secret Key  ┌───────────────────────────────────────┐      │
│              │ ••••••••••••••••••••••••••••••••••••• │      │
│              └───────────────────────────────────────┘      │
│                                 [space] Show/hide secret    │
│                                                             │
│  [ Save to Keychain ]          [ Cancel ]                   │
│                                                             │
│  ─────────────────────────────────────────────────────     │
│  ⚠  Credentials are stored in the OS keychain, never on   │
│     disk. They are zero-filled in memory after saving.      │
└─────────────────────────────────────────────────────────────┘
```

#### State

```rust
// src/app.rs

#[derive(Debug, Clone, PartialEq)]
pub enum CredentialField {
    KeyId,
    Secret,
    Save,
    Cancel,
}

impl CredentialField {
    pub fn next(&self) -> Self { /* KeyId→Secret→Save→Cancel→KeyId */ }
    pub fn prev(&self) -> Self { /* reverse cycle */ }
}

// Variant added to Modal enum:
CredentialEntry {
    /// Profile name being edited; empty string = legacy single-account mode.
    profile_name: String,
    /// Profile display label shown in the modal title.
    profile_display_name: String,
    /// Environment badge ("LIVE" / "PAPER").
    env_label: String,
    /// Contents of the Key ID text field.
    key_input: String,
    /// Contents of the Secret Key text field.
    secret_input: String,
    /// Which field currently has keyboard focus.
    focused_field: CredentialField,
    /// When `true`, secret is shown in plaintext instead of bullet characters.
    secret_visible: bool,
    /// `true` while `Command::SaveCredentials` is in-flight; disables Save button.
    saving: bool,
}
```

#### Keyboard Interaction

| Key | Focused on | Action |
|---|---|---|
| `Tab` / `↓` | Any field | Focus next field |
| `Shift+Tab` / `↑` | Any field | Focus previous field |
| Printable char | `KeyId` or `Secret` | Append to input buffer |
| `Backspace` | `KeyId` or `Secret` | Delete last character |
| `Space` | Any | Toggle `secret_visible` |
| `Enter` | `KeyId` or `Secret` | Move focus to next field |
| `Enter` | `Save` | Validate → open `Confirm` modal → emit `Command::SaveCredentials` |
| `Enter` | `Cancel` | Close modal, discard inputs |
| `Esc` | Any | Close modal, discard inputs |
| `Ctrl+U` | `KeyId` or `Secret` | Clear entire field |

#### Validation

Before the `Confirm` step, the handler checks:
- `key_input` is non-empty
- `secret_input` is non-empty
- `key_input` starts with `"PK"` (paper) or `"AK"` (live) — optional soft warning,
  not a hard error, to accommodate future Alpaca key formats

If validation fails, a transient status message is pushed: `"Key ID and secret must not be empty"`.

#### States and Transitions

```
[App starts, no credentials for active profile]
        │
        ▼
  Modal::CredentialEntry opened
        │
        ├─[Esc / Cancel]──▶ if first-run wizard: app.should_quit = true (cannot proceed)
        │                   else: modal closed, no change
        │
        ├─[Enter on Save]──▶ Modal::Confirm {
        │                      message: "Save credentials for '{profile}' to OS keychain?",
        │                      action: ConfirmAction::SaveCredentials { profile_name, key, secret }
        │                    }
        │
        └─[Confirm Yes]────▶ Command::SaveCredentials sent
                                    │
                  ┌─────────────────┴─────────────────────────┐
                  │ Success                                    │ Error
                  ▼                                            ▼
         Event::CredentialsSaved              Event::CredentialSaveError
                  │                                            │
                  ▼                                            ▼
         Event::Reconnect { config }            StatusMsg "Keychain error: {e}"
         (modal closed, data reload)            (modal stays open)
```

---

### Modal — Account Switcher (`Modal::AccountSwitcher`)

```
┌─ Switch Account ───────────────────────────────────────────┐
│                                                             │
│   ▶  Personal Live   [LIVE]   ● active                      │
│      Personal Paper  [PAPER]                                │
│      IRA             [LIVE]                                 │
│                                                             │
│  ──────────────────────────────────────────────────────    │
│  Enter: Switch    e: Edit credentials    d: Delete creds    │
│  n: New Profile   Esc: Close                                │
└─────────────────────────────────────────────────────────────┘
```

Active profile row is rendered with `selected_style()` (bold + accent background).
The `● active` badge uses the accent colour.

#### State

```rust
// Variant added to Modal enum:
AccountSwitcher {
    /// Index of the currently highlighted row (not necessarily the active profile).
    selected_idx: usize,
}
```

`App::profiles` (added below) holds the profile list; the switcher renders from
that slice. No additional state is needed beyond `selected_idx`.

#### Keyboard Interaction

| Key | Action |
|---|---|
| `j` / `↓` | Move highlight down |
| `k` / `↑` | Move highlight up |
| `g` | Jump to top |
| `G` | Jump to bottom |
| `Enter` | Emit `Command::SwitchProfile(profiles[selected_idx].name.clone())` |
| `e` | Open `Modal::CredentialEntry` for the highlighted profile |
| `d` | Open `Modal::Confirm` → `ConfirmAction::ResetCredentials { profile_name }` |
| `n` | Open `Modal::CredentialEntry` for a new (unsaved) profile — pre-populated with empty fields; profile is written to `config.toml` on save |
| `Esc` | Close modal |

#### States and Transitions

```
[User presses `S` global shortcut or navigates to Account → profile badge]
        │
        ▼
  Modal::AccountSwitcher { selected_idx: current active profile index }
        │
        ├─[Esc]─────────────────────────────────▶ modal closed
        │
        ├─[Enter on non-active profile]──────────▶ Command::SwitchProfile
        │                                                  │
        │                              ┌──────────────────┴──────────────────┐
        │                              │ resolve keychain creds               │ no creds
        │                              ▼                                      ▼
        │                   Event::Reconnect { config }        Modal::CredentialEntry
        │                   (modal closed, data reload)        (for target profile)
        │
        └─[Enter on active profile]──────────────▶ modal closed (no-op)
```

---

### Modal — Settings (`Modal::Settings`)

```
┌─ Settings ─────────────────────────────────────────────────┐
│                                                             │
│  [ App ]  [ UI ]  [ Stream ]  [ Notifications ]  [ Safety ] │
│  ─────────────────────────────────────────────────────     │
│                                                             │
│  ── App ───────────────────────────────────────────────    │
│                                                             │
│  Default Environment    ◀  live  ▶                          │
│  Refresh Interval (ms)  ┌──────┐                           │
│                       ▶ │ 5000 │ ◀                          │
│                         └──────┘                           │
│                                                             │
│  ── UI ────────────────────────────────────────────────    │
│                                                             │
│  Theme                  ◀  Default  ▶                       │
│  Show Account Panel        [✓]                              │
│  Show Watchlist            [✓]                              │
│  Show Positions            [✓]                              │
│  Show Orders               [✓]                              │
│  Default Equity Range   ◀  1D  ▶                            │
│                                                             │
│  [ Save ]                             [ Cancel ]            │
└─────────────────────────────────────────────────────────────┘
```

All sections are rendered in a single scrollable view separated by `──` dividers.
The section tab bar at the top allows jumping directly to a section. The modal
uses ~70 % of screen width and ~85 % of screen height.

#### State

```rust
// src/app.rs

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsField {
    // [app] section
    DefaultEnv,
    RefreshIntervalMs,
    // [ui] section
    Theme,
    ShowAccountPanel,
    ShowWatchlist,
    ShowPositions,
    ShowOrders,
    DefaultEquityRange,
    // [stream] section
    ReconnectMaxAttempts,
    ReconnectBackoffBaseMs,
    // [notifications] section
    FillNotificationsEnabled,
    FillNotificationTtlMs,
    StatusMessageTtlMs,
    // [safety] section
    ConfirmWatchlistRemove,
    // --- action row ---
    Save,
    Cancel,
}

impl SettingsField {
    pub fn next(&self) -> Self { /* full ordered cycle */ }
    pub fn prev(&self) -> Self { /* reverse */ }
    /// True for toggle (boolean) fields.
    pub fn is_toggle(&self) -> bool { /* ShowAccountPanel, ShowWatchlist, ... */ }
    /// True for fields edited with Left/Right cycle (enum/string fields).
    pub fn is_cyclic(&self) -> bool { /* DefaultEnv, Theme, DefaultEquityRange */ }
    /// True for numeric text-edit fields.
    pub fn is_text(&self) -> bool { /* RefreshIntervalMs, Ttl fields, backoff */ }
}

// Variant added to Modal enum:
Settings {
    /// Working copy of preferences being edited.  Not written to disk until Save.
    draft: AppPrefs,
    /// Which field is currently focused.
    focused: SettingsField,
    /// `true` while the focused text field is in character-entry mode.
    editing: bool,
    /// Raw text buffer for numeric field under edit.
    edit_buffer: String,
}
```

The `draft: AppPrefs` copy is cloned from `App::prefs` when the modal is opened.
Edits mutate `draft` in place. Pressing Save writes `draft` to `App::prefs` and
calls `Command::SavePrefs(draft)`. Pressing Cancel discards the draft.

#### Keyboard Interaction

| Key | Action |
|---|---|
| `Tab` / `j` / `↓` | Move focus to next field |
| `Shift+Tab` / `k` / `↑` | Move focus to previous field |
| `1`–`5` | Jump to section (App / UI / Stream / Notifications / Safety) |
| `←` / `→` | Cycle value for cyclic fields (DefaultEnv, Theme, DefaultEquityRange) |
| `Space` / `Enter` | Toggle boolean fields; begin/commit text edit for numeric fields |
| `Backspace` | Delete last char while editing a text field |
| `0`–`9` | Append digit while editing a numeric text field |
| `Enter` on `Save` | Commit draft → `Command::SavePrefs` → close modal |
| `Enter` on `Cancel` / `Esc` | Discard draft, close modal |

#### UX Notes

- Theme change is applied immediately to `App::current_theme` on each `←`/`→`
  press in the draft (live preview), but reverted if the user cancels.
- Boolean fields render as `[✓]` (filled) or `[ ]` (empty) using the accent color.
- Cyclic fields show a `◀ value ▶` widget.
- Proxy settings are shown read-only with the note `(restart required)`.

---

## State & Event Model

### App Struct Additions

```rust
// src/app.rs — additions to App struct

/// Named account profiles loaded from config.toml.
///
/// Empty in legacy single-account mode (no [[profiles]] in config.toml).
pub profiles: Vec<crate::prefs::ProfileConfig>,

/// Name of the currently active profile.
///
/// `None` in legacy mode. Corresponds to a `ProfileConfig::name` in `profiles`.
pub active_profile_name: Option<String>,
```

`App::new()` receives `prefs: AppPrefs` already, so `profiles` and
`active_profile_name` can be initialised from `prefs.profiles` and
`prefs.app.active_profile` without signature changes.

### New Command Variants

```rust
// src/commands.rs

/// Save API credentials for a named profile to the OS keychain.
///
/// `profile_name` is the raw keychain prefix (e.g. `"personal-live"` or
/// `"live"` in legacy mode). The async handler calls `save_keychain_pair()`
/// from `credentials.rs`.  The secret string is zero-filled after the call.
SaveCredentials {
    profile_name: String,
    key: String,
    secret: String,
},

/// Delete the keychain entries for a named profile.
///
/// Calls `credentials::reset_profile()`. Sends `Event::StatusMsg` with
/// confirmation or error.
ResetCredentials {
    profile_name: String,
},

/// Switch the active profile to the one with the given name.
///
/// The handler resolves credentials from the keychain for the target profile,
/// builds a new `AlpacaConfig`, and emits `Event::Reconnect { config }`.
/// On failure emits `Event::ProfileSwitchError`.
SwitchProfile(String),

/// Persist an edited AppPrefs snapshot to disk.
///
/// The handler calls `AppPrefs::write_to()` and emits `Event::PrefsSaved`
/// or `Event::PrefsSaveError`.
SavePrefs(AppPrefs),
```

### New Event Variants

```rust
// src/events.rs

/// Keychain save completed successfully for the named profile.
CredentialsSaved {
    profile_name: String,
},

/// Keychain save failed.
CredentialSaveError {
    profile_name: String,
    error: String,
},

/// Keychain reset completed.
CredentialsReset {
    profile_name: String,
},

/// Profile switch succeeded; carry the new config so the main loop can
/// rebuild the client and re-spawn all background tasks.
///
/// This event is handled in the **main loop** (not in `update()`), because
/// it requires cancelling running tasks and spawning new ones — operations
/// that are impossible in the synchronous `update()` function.
Reconnect {
    config: AlpacaConfig,
    /// Display name of the newly active profile (for the status message).
    profile_display_name: String,
},

/// Profile switch or credential resolve failed.
ProfileSwitchError {
    profile_name: String,
    error: String,
},

/// AppPrefs successfully written to disk.
PrefsSaved,

/// AppPrefs write failed.
PrefsSaveError(String),
```

### `update()` Changes

`Event::Reconnect` is deliberately **not** handled in `update()`. The main loop
in `src/main.rs` intercepts it before calling `update()`:

```rust
// src/main.rs — main loop (conceptual diff)
loop {
    terminal.draw(|f| ui::render(f, &mut app))?;

    match rx.recv().await {
        Some(Event::Quit) | None => break,

        // NEW: reconnect is handled here, not in update(), because it
        // requires cancelling the CancellationToken and re-spawning tasks.
        Some(Event::Reconnect { config, profile_display_name }) => {
            reconnect(
                &mut app, config, profile_display_name,
                &tx, &mut cancel, &mut client,
                &refresh_notify, &prefs,
            ).await;
        }

        Some(event) => update(&mut app, event),
    }
    // ...
}
```

The `reconnect()` helper (new function in `src/main.rs`):

```rust
async fn reconnect(
    app: &mut App,
    config: AlpacaConfig,
    profile_display_name: String,
    tx: &mpsc::Sender<Event>,
    cancel: &mut CancellationToken,
    client: &mut Arc<AlpacaClient>,
    refresh_notify: &Arc<Notify>,
    prefs: &AppPrefs,
) {
    // 1. Cancel all running background tasks
    cancel.cancel();
    // Brief yield so tasks can clean up before we replace the token
    tokio::task::yield_now().await;
    *cancel = CancellationToken::new();

    // 2. Build a new client from the new config
    *client = Arc::new(AlpacaClient::new(config.clone()));

    // 3. Update app state
    app.config = config.clone();
    app.clear_data();   // new helper: zeroes positions/orders/account/quotes/etc.
    app.push_status(StatusMessage::persistent(
        format!("Connecting to {}…", profile_display_name)
    ));

    // 4. Re-spawn all background tasks (same as main() startup block)
    tokio::spawn(handlers::input::run(tx.clone(), cancel.clone()));
    tokio::spawn(handlers::rest::run(
        tx.clone(), cancel.clone(), client.clone(),
        refresh_notify.clone(), prefs.clone(),
    ));
    tokio::spawn(handlers::commands::run(
        command_rx, tx.clone(), client.clone(),
        refresh_notify.clone(), cancel.clone(),
    ));
    tokio::spawn(alpaca_trader_rs::stream::market::run(
        tx.clone(), cancel.clone(), config.clone(),
        symbol_rx, prefs.clone(),
    ));
    tokio::spawn(alpaca_trader_rs::stream::account::run(
        tx.clone(), cancel.clone(), config.clone(), prefs.clone(),
    ));

    tokio::spawn(handlers::rest::poll_once(tx.clone(), client.clone()));
}
```

> **Note on `command_rx` / `symbol_rx`**: because the channel receivers are moved
> into the spawned tasks they are consumed. On reconnect, new channels must be
> created. The senders (`command_tx`, `symbol_tx`) stored on `App` must also be
> replaced. This is achieved by declaring them `mut` in `main()` and replacing
> them as part of `reconnect()`. `App::command_tx` and `App::symbol_tx` are
> updated to the new senders after channel creation.

`Event::CredentialsSaved`, `Event::PrefsSaved`, and `Event::PrefsSaveError` are
handled in `update()` as simple status message pushes and modal close operations.

---

## Startup Flow Changes

### Current Flow

```
main()
  │
  ├── load AppPrefs
  ├── determine AlpacaEnv (CLI --paper or prefs.app.default_env)
  ├── credentials::resolve(env)          ← may print/prompt on stderr/stdin
  ├── AlpacaConfig::from_credentials()
  ├── enable_raw_mode()
  ├── spawn tasks
  └── main loop
```

### New Flow

```
main()
  │
  ├── load AppPrefs
  ├── determine startup_env (CLI --paper or prefs.app.default_env)
  ├── [PROFILE MODE: prefs.profiles non-empty]
  │     ├── select active_profile from prefs.app.active_profile
  │     │   (fall back to profiles[0] if unset)
  │     ├── try credentials::resolve_profile(&profile)
  │     │     ├── [env vars] → Ok(creds)
  │     │     ├── [keychain] → Ok(creds)
  │     │     └── [no creds] → Ok(None)   ← NEW: returns None instead of prompting
  │     │
  │     ├── if Ok(Some(creds)) → build AlpacaConfig, continue
  │     └── if Ok(None) → launch TUI, then open Modal::CredentialEntry for profile
  │
  ├── [LEGACY MODE: prefs.profiles empty]
  │     ├── credentials::resolve(env)     ← unchanged; may prompt before raw mode
  │     └── AlpacaConfig::from_credentials()
  │
  ├── enable_raw_mode()
  ├── spawn tasks
  │     ├── [PROFILE MODE, no creds] → REST tasks start in "standby" state
  │     │   and wait for Event::Reconnect before polling
  │     └── [all other cases] → normal task startup
  │
  └── main loop
        ├── [PROFILE MODE, no creds at startup]
        │     → immediately open Modal::CredentialEntry before first render
        └── normal event loop
```

#### `credentials::resolve_profile()` — New Function

```rust
// src/credentials.rs

/// Resolve credentials for a named profile without prompting.
///
/// Returns `Ok(Some(creds))` if env vars or keychain provide credentials.
/// Returns `Ok(None)` if no credentials are found (caller opens CredentialEntry).
/// Returns `Err` only on hard keychain errors.
pub fn resolve_profile(profile: &ProfileConfig) -> Result<Option<ResolvedCredentials>> {
    let env = match profile.env.as_str() {
        "paper" => AlpacaEnv::Paper,
        _       => AlpacaEnv::Live,
    };
    let prefix = &profile.name;  // e.g. "personal-live"
    let (env_prefix, default_ep) = match env {
        AlpacaEnv::Live  => ("LIVE_ALPACA", DEFAULT_LIVE_ENDPOINT),
        AlpacaEnv::Paper => ("PAPER_ALPACA", DEFAULT_PAPER_ENDPOINT),
    };
    let endpoint = std::env::var(format!("{env_prefix}_ENDPOINT"))
        .unwrap_or_else(|_| default_ep.to_string());

    // Step 1a: unified env vars
    // ... (same as resolve())

    // Step 1b: per-environment env vars
    // ... (same as resolve())

    // Step 2: keychain lookup using profile name as prefix
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    match try_keychain_load(prefix) {
        Ok(Some((key, secret))) => return Ok(Some(ResolvedCredentials {
            endpoint, key, secret, env,
        })),
        Ok(None) => {}    // not found — caller opens CredentialEntry
        Err(KeychainStatus::Unavailable(msg)) => {
            tracing::warn!("keychain unavailable for profile {}: {}", prefix, msg);
        }
        Err(KeychainStatus::Hard(e)) => return Err(e),
    }

    Ok(None)   // no credentials found; caller opens CredentialEntry
}
```

`credentials::resolve()` (the existing function) is unchanged to maintain
legacy mode behaviour.

---

## Security Model

### Secret Display

- The `secret_input` buffer is rendered as `•` characters when `secret_visible == false`.
- When `secret_visible == true`, the secret is shown in plaintext — the modal title
  flashes `[secret visible]` in the neutral/yellow colour.
- The secret is **never** written to any log file. `tracing` calls involving
  `ResolvedCredentials` log only the profile name and env label, never the key or secret.

### Memory Safety

Add the `zeroize` crate (already implied by the `secrecy` ecosystem, zero new deps if
`secrecy` is chosen) to zero credential buffers after use:

```toml
# Cargo.toml
zeroize = { version = "1", features = ["derive"] }
```

```rust
use zeroize::Zeroize;

// In the CredentialEntry modal handler, after SaveCredentials is dispatched:
state.key_input.zeroize();
state.secret_input.zeroize();

// In handlers/commands.rs, SaveCredentials handler, after keychain write:
key.zeroize();
secret.zeroize();
```

`ResolvedCredentials` is short-lived (constructed in the command handler, used
once to build `AlpacaConfig`, then dropped). `AlpacaConfig::key` and
`AlpacaConfig::secret` are currently plain `String`s; they are passed by clone
into each background task. Wrapping them in `zeroize::Zeroizing<String>` is
desirable but a larger refactor — tracked as a follow-up in Phase 3.

### Confirmation Gating

Every destructive credential operation goes through a `Modal::Confirm` step:
- Save to keychain: `"Save credentials for '{profile}' to OS keychain?"`
- Delete from keychain: `"Delete stored credentials for '{profile}'? You will be
  prompted to re-enter them next time."`
- Switch profile: if current profile has unsaved orders, add `"You have open orders
  on {current_profile}. Switch anyway?"` warning.

### Key Validation

The handler rejects `key_input` values that are empty or contain whitespace, and
warns (soft) when the prefix doesn't match the expected Alpaca format for the
selected environment (`PK…` for paper, `AK…` for live). This guards against common
copy-paste mistakes but does not prevent entry of unconventional key formats.

---

## Implementation Phases

### Phase 1 — Settings Modal

**Scope:** `Modal::Settings` only. No credential handling. No profile support.

**Why first:** Lowest risk. Exercises the modal → command → disk-write → event
pipeline without touching credentials or reconnect logic. Delivers immediate user
value (persist theme choice, change poll interval without restart).

**Deliverables:**

1. `SettingsField` enum + `Modal::Settings` variant in `src/app.rs`
2. `Command::SavePrefs(AppPrefs)` in `src/commands.rs`
3. `Event::PrefsSaved` and `Event::PrefsSaveError(String)` in `src/events.rs`
4. Settings modal renderer in `src/ui/modals.rs`
5. Settings modal keyboard handler in `src/input/modal.rs`
6. `handlers/commands.rs`: match arm for `Command::SavePrefs` — calls
   `AppPrefs::write_to(&AppPrefs::default_path().unwrap())`
7. `update.rs`: match arms for `Event::PrefsSaved` / `Event::PrefsSaveError`
8. `update.rs` `handle_key`: bind `,` or `S` (shift) as global shortcut to open
   `Modal::Settings`
9. Theme live-preview: on each `←`/`→` press on the Theme field in the draft,
   call `app.current_theme = Theme::from_str(&draft.ui.theme)`; revert on Cancel

**Tests:**

- `settings_draft_cloned_from_app_prefs` — open modal, verify `draft == app.prefs`
- `settings_cancel_does_not_mutate_prefs` — edit a field, cancel, verify `app.prefs`
  unchanged
- `settings_save_writes_draft_to_app_prefs` — edit, save, verify `app.prefs` updated
- `save_prefs_command_writes_file` — integration test using `tempfile`, verify TOML
  round-trips
- `theme_live_preview_reverted_on_cancel` — open modal, cycle theme, cancel, verify
  `app.current_theme == original`

---

### Phase 2 — In-App Credential Entry

**Scope:** `Modal::CredentialEntry` and the first-run TUI wizard. No multi-profile
support yet — this phase works in legacy mode only (single live/paper account).

**Why second:** Eliminates the only reason a user must restart the app (stale
credentials). Also lays the pipe (SaveCredentials command, reconnect event, task
re-spawn) that Phase 3 reuses for profile switching.

**Deliverables:**

1. `CredentialField` enum + `Modal::CredentialEntry` variant in `src/app.rs`
2. `Command::SaveCredentials` and `Command::ResetCredentials` in `src/commands.rs`
3. `Event::CredentialsSaved`, `Event::CredentialSaveError`, `Event::CredentialsReset`,
   `Event::Reconnect { config, profile_display_name }` in `src/events.rs`
4. Credential entry modal renderer in `src/ui/modals.rs`
5. Credential entry keyboard handler in `src/input/modal.rs`
6. `handlers/commands.rs` match arms for `SaveCredentials` (async keychain write),
   `ResetCredentials` (async keychain delete)
7. `credentials::save_keychain_pair_async()` — thin async wrapper that offloads
   the blocking `keyring` call to `tokio::task::spawn_blocking`
8. `reconnect()` helper function in `src/main.rs`
9. `Event::Reconnect` interception in the main loop (before `update()` call)
10. `App::clear_data()` helper (zeroes positions, orders, account, quotes, etc.)
11. First-run wizard: in `main()`, after spawning tasks, if credentials could not
    be resolved pre-TUI in profile mode, push `Modal::CredentialEntry` onto `app.modal`
12. Global keybinding `Ctrl+K` → open `Modal::CredentialEntry` for the active env
13. `zeroize` added to `Cargo.toml`; zero `key_input`/`secret_input` after dispatch

**Tests:**

- `credential_entry_backspace_clears_last_char` — unit test on modal key handler
- `credential_entry_esc_closes_without_save` — modal closed, no command sent
- `credential_entry_secret_toggle` — space toggles `secret_visible`
- `save_credentials_command_calls_keychain` — mock `save_keychain_pair`, verify called
  with correct profile prefix and key/secret values
- `reconnect_clears_app_data` — after `App::clear_data()`, all data fields are empty
- Integration test (with `wiremock`): new credentials → reconnect → REST poll delivers
  fresh `AccountInfo`

---

### Phase 3 — Multi-Profile Support

**Scope:** `ProfileConfig`, `[[profiles]]` TOML schema, `Modal::AccountSwitcher`,
profile-namespaced keychain, profile switching with reconnect, `--reset` profile
option in CLI.

**Why last:** Requires config schema changes (breaking for `to_toml_string()`),
the most new state on `App`, and depends on the reconnect pipeline built in Phase 2.

**Deliverables:**

1. `ProfileConfig` struct in `src/prefs.rs`; `AppSection::active_profile: Option<String>`;
   `AppPrefs::profiles: Vec<ProfileConfig>`
2. `AppPrefs::to_toml_string()` updated to serialise `[[profiles]]` and
   `active_profile` (only when profiles are non-empty — preserves legacy output)
3. `credentials::resolve_profile(profile: &ProfileConfig)` in `src/credentials.rs`
4. `AccountSwitcher` variant in `Modal` enum
5. Account switcher renderer in `src/ui/modals.rs`
6. Account switcher keyboard handler in `src/input/modal.rs`
7. `Command::SwitchProfile(String)` in `src/commands.rs`
8. `handlers/commands.rs`: `SwitchProfile` handler — resolves keychain creds for
   target profile, emits `Event::Reconnect` or opens `Modal::CredentialEntry`
9. `App::profiles` and `App::active_profile_name` fields; initialised in `App::new()`
10. Header badge updated: shows `profile.display_name [ENV]` when in profile mode
11. `App::active_profile()` helper — returns `Option<&ProfileConfig>` for the active name
12. `--reset` CLI flag extended to accept profile names (in addition to `paper`/`live`)
13. Startup flow: call `resolve_profile()` instead of `resolve()` when profiles exist
14. `AppPrefs::add_profile()` / `AppPrefs::remove_profile()` helpers used by the
    account switcher's `n` (new) and `d` (delete) commands

**Tests:**

- `profile_config_round_trips_toml` — serialise `AppPrefs` with profiles, parse back
- `legacy_toml_no_profiles_is_compatible` — existing `config.toml` parses with zero
  profile entries
- `resolve_profile_returns_none_when_no_keychain_entry` — mock keychain returns
  `NoEntry`; verify `Ok(None)` returned
- `account_switcher_enter_sends_switch_command` — unit test for key handler
- `switch_profile_reconnects_with_new_config` — integration test: emit
  `Command::SwitchProfile`, verify `Event::Reconnect` emitted with correct env
- `header_badge_shows_profile_display_name` — unit test on header renderer

---

## File Changes Inventory

| File | Change |
|---|---|
| `src/prefs.rs` | Add `ProfileConfig`, `AppSection::active_profile`, `AppPrefs::profiles`, update `to_toml_string()`, add `add_profile()` / `remove_profile()` helpers |
| `src/app.rs` | Add `Modal::CredentialEntry`, `Modal::AccountSwitcher`, `Modal::Settings`; add `CredentialField`, `SettingsField` enums; add `App::profiles`, `App::active_profile_name`, `App::clear_data()`, `App::active_profile()` |
| `src/commands.rs` | Add `SaveCredentials`, `ResetCredentials`, `SwitchProfile`, `SavePrefs` variants |
| `src/events.rs` | Add `CredentialsSaved`, `CredentialSaveError`, `CredentialsReset`, `Reconnect`, `ProfileSwitchError`, `PrefsSaved`, `PrefsSaveError` variants |
| `src/credentials.rs` | Add `resolve_profile()`, `save_keychain_pair_async()`, `reset_profile()` functions; keep `resolve()` unchanged |
| `src/handlers/commands.rs` | Add match arms for `SaveCredentials`, `ResetCredentials`, `SwitchProfile`, `SavePrefs` |
| `src/update.rs` | Add match arms for new events; add `Modal::CredentialEntry`, `Modal::AccountSwitcher`, `Modal::Settings` open shortcuts in `handle_key()` |
| `src/input/modal.rs` | Add match arms for `Modal::CredentialEntry`, `Modal::AccountSwitcher`, `Modal::Settings` |
| `src/ui/modals.rs` | Add renderers for the three new modals; register `popup_area` sizes for hit-test; add popup_area constants for the new modal sizes |
| `src/main.rs` | Add `Event::Reconnect` intercept before `update()` call; add `reconnect()` async helper; update startup flow for profile mode; extend `--reset` CLI flag |
| `src/ui/dashboard.rs` | Update header badge to show profile display name when in profile mode |
| `Cargo.toml` | Add `zeroize = { version = "1", features = ["derive"] }` |
| `tests/` | New integration tests for credential save, profile switch, prefs persistence |

---

## Test Strategy

### Unit Tests (in-module `#[cfg(test)]`)

| Module | What to test |
|---|---|
| `src/prefs.rs` | `ProfileConfig` TOML round-trip; `AppPrefs` with `[[profiles]]` parses correctly; legacy TOML (no profiles) still loads; `to_toml_string()` emits `[[profiles]]` only when non-empty; `add_profile` / `remove_profile` maintain sorted order |
| `src/credentials.rs` | `resolve_profile` returns `Ok(None)` when keychain empty (mock via `temp-env`); `resolve_profile` prefers unified env vars; `save_keychain_pair_async` compiles (integration only, no CI keychain) |
| `src/app.rs` | `App::clear_data()` zeroes all data vectors; `App::active_profile()` returns the matching profile; `CredentialField::next/prev` cycles correctly; `SettingsField::is_toggle/is_cyclic/is_text` classification |
| `src/ui/theme.rs` | Already has tests; verify `Theme::from_str` is the canonical parse used by both `Settings` draft and `App::current_theme` |

### Integration Tests (`tests/`)

| Test | Approach |
|---|---|
| `test_save_prefs_writes_toml` | Create `AppPrefs`, mutate one field, call `write_to(tempfile)`, parse back with `load_from()`, assert equality |
| `test_settings_modal_save_command` | Build test app, open `Modal::Settings`, mutate a field in draft, press Save, verify `command_tx` receives `Command::SavePrefs` |
| `test_credential_entry_dispatches_command` | Open `Modal::CredentialEntry`, type key/secret, press Save, confirm, verify `Command::SaveCredentials` dispatched with correct profile and values |
| `test_reconnect_clears_state_and_respawns` | Start app with `wiremock` server, emit `Event::Reconnect { config }`, verify `App::account` is `None` after reconnect and a new REST poll populates it |
| `test_account_switcher_no_creds_opens_credential_entry` | `Command::SwitchProfile` for profile with no keychain entry → verify `Modal::CredentialEntry` is opened |
| `test_profile_toml_backward_compat` | Parse existing v0.6 `config.toml` fixture (no `[[profiles]]`), verify `profiles` is empty and startup succeeds |

### Mock Patterns

- **Keychain**: use `temp-env` to set `{PREFIX}_KEY` / `{PREFIX}_SECRET` env vars
  so `resolve_profile()` returns a result without touching the real keychain. The
  `keyring` crate is never called in unit tests.
- **REST**: existing `wiremock` test infrastructure in `tests/` continues unchanged.
- **TUI render**: do not test pixel output; test that the correct `Modal` variant is
  set on `App` and the correct `Command` is sent through the channel. The render
  functions are exercised by visual inspection and snapshot tests as a stretch goal.

---

## Open Questions

1. **`command_rx` / `symbol_rx` on reconnect**: Both channel receivers are moved
   into their respective tasks on startup, making them unavailable for re-use on
   reconnect. The simplest resolution is to create new channels in `reconnect()`
   and update `app.command_tx` / `app.symbol_tx` to the new senders. However, any
   in-flight `Command` in the old `command_rx` will be lost. Should we drain the
   old channel before replacing it? Or is it acceptable to drop pending commands
   on profile switch (they would be for the wrong account anyway)?

2. **`--reset` for profiles**: The existing `--reset live` / `--reset paper` CLI
   interface conflicts with profile names (e.g., `--reset personal-live`). Options:
   - Accept any string: `--reset <profile-name-or-env>` and try both legacy and
     profile keychain namespaces.
   - Add `--reset-profile <name>` as a separate flag.
   Recommendation: unified `--reset <name>` where `paper` and `live` are treated as
   legacy aliases, any other string as a profile name.

3. **Profile creation UI**: Phase 3 allows creating a new profile from the
   `AccountSwitcher` via `n`. This requires a form with `name`, `display_name`, and
   `env` fields before opening `CredentialEntry`. Should this be a separate
   `Modal::NewProfile` or extend `CredentialEntry` with extra fields? A separate
   modal keeps each modal focused but adds a new renderer; extending `CredentialEntry`
   reduces file count but increases its complexity.

4. **`zeroize` scope for `AlpacaConfig`**: Wrapping `AlpacaConfig::key` and
   `AlpacaConfig::secret` in `Zeroizing<String>` would require changes across
   all callers (stream tasks, REST client headers). The `reqwest` client copies
   header values into an internal buffer anyway, so zeroing `AlpacaConfig::secret`
   does not zero the copy in the HTTP client. A full secret-hygiene pass is a larger
   cross-cutting refactor best scoped as a separate issue.

5. **Proxy settings in Settings modal**: The `[proxy]` section contains `Option<String>`
   fields. Since proxy changes require rebuilding the `reqwest::Client`, they cannot
   be hot-reloaded without a reconnect. The Settings modal should mark these fields
   as `(restart required)` and skip calling `Command::SavePrefs`-triggered reconnect
   automatically. Should proxy settings appear in the modal at all in Phase 1, or
   wait until the reconnect pipeline is in place (Phase 2)?

6. **Multiple profiles with the same `name`**: The schema does not enforce uniqueness
   on `ProfileConfig::name`. Should `AppPrefs::add_profile()` return an error on
   duplicate names, or silently replace the existing entry? The keychain namespace
   collision would silently overwrite the old credentials.

7. **Paper mode watchlist unavailability notice**: Currently `Event::WatchlistUnavailable`
   is a one-shot event. When switching from a live profile (which has watchlists) to a
   paper profile, `app.watchlist_unavailable` should be reset to `false` in
   `App::clear_data()` and re-set when the REST handler fires the event again for the
   new profile.
```

---

