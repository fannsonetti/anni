# anni

**anni** is a lightweight unofficial daily school planner for the Icelandic school system **INNA** (`nam.inna.is`).

Open **Today** to see your next class and room, unfinished work ordered by deadline, and today's schedule. **My work** combines assignments, exams, homework, INNA to-dos, and personal tasks. Add study steps or reminders, filter by today / the next seven days / done, and search by task or course.

Checkmarks are local planning progress; assignments must still be submitted in INNA. Work already handed in or graded in INNA appears under Done. Items without a recognizable deadline stay visible under All open, with the original deadline text retained when available. Dates and times use Icelandic time (UTC).

## English interface and grades

Anni uses English throughout its navigation, dates, settings, desktop widgets, and helper screens. Original school announcements, assignment titles, course codes, and names are kept as supplied by INNA. Numeric grades use decimal points. Personal task dates accept `dd.mm.yyyy` and ISO dates.

The home page puts your unfinished work first, with Upcoming, Due today, and Overdue views. Three quick summaries show today's deadlines, overdue work, and all open tasks; click a summary to open its tasks. A dedicated side panel shows your next class, today's focus time, and a compact course snapshot. The seven-day deadline strip opens the selected day's tasks. Content stays centered on large displays and stacks into a single column in smaller windows.

**Courses & grades** includes courses found in either assignments or the timetable. Its current weighted average is the sum of numeric grades multiplied by positive weights, divided by the graded weight. Ungraded work is not counted as zero; zero-weight items are excluded. Missing weights, nonnumeric weighted grades, unknown categories, and inconsistent totals are flagged instead of guessed.

The published [Flensborg curriculum, page 18](https://www.flensborg.is/static/files/PDF_allskonar/almennur-hluti_yfirlesinn-2020.pdf#page=18) states the general final-grade requirement of **5 (45%)**. Anni compares the unrounded current average with **4.50**. This is provisional progress, not confirmation of a final grade or of individual exam and submission requirements. These may be specified in each course's syllabus. [Course descriptions](https://www.flensborg.is/is/namid/nam-og-namshaettir/afangalysingar) are linked in the app. Sources checked 18 October 2026.

Your downloaded data, personal tasks, and checkmarks are saved in SQLite after changes and restored on startup. You can use your plan before connecting, offline, or after a session expires. Refresh failures keep the previous data and appear above the planner, with details in Settings. A successful empty response replaces the old list. Each INNA account keeps its own personal plan on the device.

> **Disclaimer:** This project is not affiliated with, endorsed by, or supported by INNA or InfoMentor. It uses undocumented/private INNA web endpoints that may change without notice.

## Platforms

- **Windows 10/11 x64** — primary development target
- **Raspberry Pi OS 64-bit ARM64** — primary deployment target (1 GB RAM)

## Windows installer

Download **Anni-0.1.0-windows-x64-setup.exe** from the [latest release](https://github.com/fannsonetti/anni/releases/latest), then run it. Setup installs Anni and its INNA sign-in helper for your Windows account, adds Start menu shortcuts for the planner and desktop widgets, and offers an optional desktop shortcut. No administrator account is required. Reopen **Anni** from a shortcut to change widget settings or quit the app.

Setup checks for Microsoft Edge WebView2 and installs it when missing, using Microsoft's signed Evergreen bootstrapper. Internet access is needed for that prerequisite. Existing WebView2 installations are reused. See [Microsoft's deployment documentation](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

The required Microsoft Visual C++ runtime DLLs are bundled alongside the app using [Microsoft's app-local deployment method](https://learn.microsoft.com/en-us/cpp/windows/choosing-a-deployment-method). Installer builds require the Visual Studio C++ tools and their x64 redistributable files; `-CrtDir` can select that directory explicitly.

Uninstall through **Windows Settings → Apps → Installed apps → Anni**. Saved sessions, downloaded schoolwork, and your personal plan remain in `%LOCALAPPDATA%/inna-client`; uninstalling removes the installed app's Windows startup entry if it was enabled. Releases currently have no code-signing certificate.

To build the installer yourself, install [Inno Setup 6.3 or newer](https://jrsoftware.org/isdl.php), then run:

```powershell
./scripts/build-installer.ps1
# Or download and verify the compiler into artifacts/installer:
./scripts/build-installer.ps1 -BootstrapCompiler
# Verify installation, upgrade, startup, shortcuts, and uninstall on a clean account:
./scripts/test-installer.ps1
```

The script builds both release executables, verifies the Microsoft bootstrapper's Authenticode signature, and writes the installer and SHA-256 checksum to `dist/`. Use `-SkipBuild` to package existing release binaries, or `-IsccPath` to select a compiler. GitHub Actions builds and tests the installer on pushes and pull requests. Pushing a `v*` tag also publishes the installer as a release; keep the tag version aligned with `crates/inna-app/Cargo.toml`.

## Architecture

```
Native Rust process
    ├── egui/eframe UI
    ├── reqwest HTTP client (cookies, XSRF)
    ├── SQLite cache (offline-first)
    ├── Background sync scheduler
    └── Separate auth helper process (webview, exits after login)
```

No permanent browser process runs during normal use.

### Workspace structure

| Crate | Purpose |
|-------|---------|
| `inna-core` | INNA API client, models, session, HTML parsing |
| `inna-cache` | SQLite persistence layer |
| `inna-auth` | Separate auth helper binary (WebView2 / WebKitGTK) |
| `inna-app` | Native egui application |

## Authentication

Login happens on INNA's real website. Two modes:

1. **This device** — temporary WebView2 / WebKitGTK window
2. **Phone (QR)** — for machines that cannot open websites interactively. The app shows a QR code; your phone (same Wi‑Fi) opens a local relay, you sign in on INNA there, and the session is captured. Requires outbound HTTPS from the device (same as normal sync).

The app never stores your password. After login, session cookies are saved and the auth helper exits completely.

Session cookies are stored securely:
- **Windows:** Local app data with restrictive permissions
- **Linux:** File with `0600` permissions (keyring support planned)

## Building

### Windows

```bash
cargo build --release
```

Produces `target/release/anni.exe` and `target/release/inna-auth.exe`.

INNA sign-in requires WebView2 Runtime. The Windows installer checks and installs this prerequisite when needed; for manual builds, install it from [Microsoft](https://developer.microsoft.com/en-us/microsoft-edge/webview2/).

### Raspberry Pi OS (64-bit)

Install dependencies:

```bash
sudo apt update
sudo apt install -y \
    build-essential \
    pkg-config \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libssl-dev \
    libsqlite3-dev \
    libxkbcommon-dev \
    libwayland-dev \
    libxrandr-dev \
    libxcursor-dev \
    libxi-dev \
    libx11-dev \
    libgl1-mesa-dev
```

Build natively on the Pi:

```bash
cargo build --release
```

### Release profile

Optimized for size and efficiency on constrained hardware:

```toml
[profile.release]
opt-level = "s"
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## Usage

### Normal mode

```bash
anni
```

### Mock mode (no INNA connection, fake data)

```bash
anni --mock
```

### Fullscreen (Raspberry Pi kiosk)

```bash
anni --fullscreen
```

### Desktop schedule, homework, and clock (Windows)

Open **Settings → Desktop**, then enable **Show desktop widgets**. Your schedule sits on the left, homework on the right, and a minimal digital clock in the center. Both side widgets have matching widths and alignment. Borders are off by default; **Show widget borders** adds an optional subtle outline to all three widgets. Native title bars, shadows, and close buttons are suppressed separately. Choose **Clear**, **Subtle**, or **Contrast** under **Colors & background** to adjust the optional background tint. Windows DWM composition preserves per-pixel transparency so Wallpaper Engine remains visible behind them. They sit beneath application windows on the primary display and adapt to its working area and Windows display scaling; on smaller displays, a compact clock sits above the two columns.

The schedule shows class times, subjects, and rooms for the current school day while classes remain, then advances to the next day present in your downloaded timetable, including across weekends. Current and next classes are marked. The homework widget shows **Heimavinna and course assignments (Verkefni)** in deadline order, including overdue and undated work. Exams, personal tasks, and INNA reminders are optional additions in Settings. Both lists scroll with the mouse wheel or scrollbar. Widgets only display information: text cannot be selected, clicking headings or rows does nothing, and there are no three-dot menus. Manage task checkmarks in **My work** in the planner. Saved schoolwork appears before the network connects; the existing five-minute INNA refresh continues in the background, including when only the clock is visible. The clock uses 24-hour Icelandic time (UTC). Idle lists refresh every 15 seconds and update immediately after local changes. The clock updates on second boundaries when seconds are enabled, with a 15-second background service check otherwise. Native window geometry and styling update only when needed. Appearance changes are saved after a short pause in editing.

**Settings → Desktop** offers six themes: **Minimal**, **Arc**, **Iron Man**, **Forest**, **Lavender**, and **Sunset**. Arc and Iron Man use a single fine broken reactor ring without glow or decorative ticks. Each theme coordinates text, secondary text, accent, and background colors. Choose the theme accent, one of four preset accents, or your own color with the custom color picker. Digital, minimal reactor, and analog clocks are available with date and seconds switches. You can also change widget visibility, width, height, text size, vertical position, row spacing, corner rounding, background opacity, and task course-code/deadline visibility. Theme selection resets the accent and clock style to that preset; other layout and content choices stay in place. Appearance changes save automatically. **Reset appearance** restores defaults.

Sync reads the enrolled course list and visits each course's **Verkefni og próf** endpoint, in addition to global projects, global Heimavinna, course Heimavinna, and course homework calendars. It reads INNA's `text` / `date` and `handInFullDate` fields, excludes teaching-goal-only rows, empty HTML, generic Homework/Heimavinna headings, and evaluations from the homework queue, and combines duplicate homework. Actual instructions accompanying a generic homework heading and dated assignments are retained. Calendar checks cover the previous 90 days and next 120 days; the regular Heimavinna and course assignment lists remain unrestricted. Local checkmarks survive refreshes, restarts, and matching homework moving between views. A failed source retains its saved rows and reports a refresh warning; a successful empty response clears that source.

Choose **Show desktop widgets** or close the main window to leave the widgets running. Launching Anni again from its shortcut reopens the existing planner. Use **Settings → Desktop** to change appearance, swap sides, disable **Show desktop widgets**, or **Quit Anni** to close all Anni windows and exit completely.

Enable **Open desktop widgets when I sign in to Windows** to register the current executable for your Windows account. Keep the executable at that location; after moving it, turn this setting off and on from the new location. It launches directly into desktop mode, without leaving the planner open. This option is disabled in mock mode. Desktop preferences are stored separately from account data in `%LOCALAPPDATA%/inna-client/desktop.json`.

```bash
anni --desktop
# Temporary sample widgets, without changing real tasks or preferences:
anni --mock --desktop
```

`--desktop` and `--fullscreen` are mutually exclusive. Rainmeter can be closed separately once you enable Anni's clock; Anni does not modify Rainmeter or Wallpaper Engine.

## Raspberry Pi Autostart

Create `~/.config/autostart/anni.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=anni
Exec=/path/to/anni --fullscreen
X-GNOME-Autostart-enabled=true
```

Or use a user systemd service:

```ini
# ~/.config/systemd/user/anni.service
[Unit]
Description=anni
After=graphical-session.target

[Service]
ExecStart=/path/to/anni --fullscreen
Restart=on-failure

[Install]
WantedBy=graphical-session.target
```

```bash
systemctl --user enable anni
systemctl --user start anni
```

## Navigation and branding

anni is a separate, unofficial way to view INNA. INNA remains the name of the service and data source. A mint open-book mark with a completion tick appears in the sidebar, compact header, and native window icon; its reusable SVG is in `crates/inna-app/assets/anni.svg`.

The desktop sidebar groups daily planning separately from school information, with short descriptions and a clear current-page indicator. Short windows keep the same sidebar with a scrollable menu. Smaller windows show **Today**, **My work**, **Timetable**, and a **More** menu containing every page. Settings stays directly accessible from the desktop sidebar and the compact menu. Back and forward buttons (also **Alt+Left / Alt+Right**) return through page links and announcement details. Clicking the anni logo opens Today. An always-visible page label shows your current location, and the add-task action is available on every page.

The executable is now `anni`; the internal `inna-*` crate names and existing `inna-client` data directory stay compatible with saved sessions and planner data.

## Features

- **Always dark** — fixed dark appearance, including native window chrome, inputs, menus, and every screen; does not follow the operating system theme
- **Today** — personalized dashboard, next class (including the next school day), room, urgent work, and today's schedule; clickable seven-day workload chart opens the deadlines for a selected day
- **My work** — combined deadline queue, local checkmarks, editable personal tasks, quick due dates, undo for the latest deletion, filters and search
- **Focus** — 25- or 50-minute study sessions, five-minute breaks, task selection, pause/resume, progress ring, and saved session history with today's study minutes; a running or paused timer is accessible across pages
- **Timetable** — day-by-day schedule, current/next lesson indicators
- **Announcements** — list and detail views with HTML-to-text conversion, full-text search, and an unread-in-INNA filter
- **Courses** — assignments, exams, and grades grouped by course
- **Offline mode** — downloaded data and personal plans restored from SQLite without blocking on login
- **Refresh** — manual refresh and automatic updates every five minutes; failed updates retain saved data
- **Touchscreen** — large touch targets, readable at arm's length

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Session expired | Click "Reconnect" — opens login window |
| WebView2 missing (Windows) | Install from microsoft.com/edge/webview2 |
| WebKitGTK missing (Linux) | `sudo apt install libwebkit2gtk-4.1-dev` |
| INNA unavailable | App shows cached data, retries automatically |
| Blank timetable | Check connection status, try manual refresh |
| No assignments shown | INNA may return empty — this is normal |
| Cannot save changes | Keep the app open; use Retry saving once local storage is available |
| Database corruption | Delete `~/.local/share/inna-client/inna-cache.db` |

## Target memory usage

```
< 150 MB preferred (normal runtime, no webview)
< 200 MB acceptable
```

The login webview may use more during authentication, but exits completely afterward.

## Development checks

```bash
cargo test --workspace --offline
# Windows native lifecycle regression (opens temporary mock windows):
cargo test -p inna-app --offline native_hide_and_quit_from_a_hidden_planner -- --ignored --test-threads=1
cargo run -p inna-app --offline -- --mock
# Read-only live connection and homework-source check (uses your saved sign-in):
cargo run -p inna-app --offline -- --test-session
```

Mock mode uses sample deadlines relative to today and does not read or write your real planner. Changes in mock mode are temporary. Automated checks cover deadline formats, overdue sorting, completed work, account isolation, weekend lesson selection, SQLite persistence, and refresh failure handling. Live INNA behavior still depends on its private endpoints and your authenticated session.

Focus timers continue while you navigate within the app. Closing the app discards a running or paused session; only completed study sessions are saved. Breaks do not count toward study minutes. Switching INNA accounts stops the active timer and keeps completed session history with the corresponding account. Task checkmarks and announcement filters are local views and do not change submission or read status in INNA.
