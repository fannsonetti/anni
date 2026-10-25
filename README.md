# anni

**anni** is a lightweight unofficial daily school planner for the Icelandic school system **INNA** (`nam.inna.is`).

Open **Today** to see your next class and room, unfinished work ordered by deadline, and today's schedule. **My work** combines assignments, exams, homework, INNA to-dos, and personal tasks. Add study steps or reminders, filter by today / the next seven days / done, and search by task or course.

Checkmarks are local planning progress; assignments must still be submitted in INNA. Work already handed in or graded in INNA appears under Done. Items without a recognizable deadline stay visible under All open, with the original deadline text retained when available. Dates and times use Icelandic time (UTC).

## English interface and grades

Anni uses English throughout its navigation, dates, settings, desktop widgets, and helper screens. Original school announcements, assignment titles, course codes, and names are kept as supplied by INNA. Numeric grades use decimal points. Personal task dates accept `dd.mm.yyyy` and ISO dates.

The home page puts your unfinished work first, with Upcoming, Due today, and Overdue views. Three quick summaries show today's deadlines, overdue work, and all open tasks; click a summary to open its tasks. A dedicated side panel shows your next class, today's focus time, and a compact course snapshot. The seven-day deadline strip opens the selected day's tasks. Content stays centered on large displays and stacks into a single column in smaller windows.

**Courses & grades** includes courses found in either assignments or the timetable. Its current weighted average is the sum of numeric grades multiplied by positive weights, divided by the graded weight. INNA's assignment weights already include the category contribution (including a 40% / 60% split), so categories do not apply a second multiplier or prevent calculation. Each category also shows its own average and listed/graded weights, matched to the refreshed assignments by ID. Ungraded work is not counted as zero; zero-weight items are excluded. Missing weights, nonnumeric weighted grades, and inconsistent totals are flagged instead of guessed. FJÁR/FJAR courses display as Hagfræði, and HMN/HMNM as Homework.

The published [Flensborg curriculum, page 18](https://www.flensborg.is/static/files/PDF_allskonar/almennur-hluti_yfirlesinn-2020.pdf#page=18) states the general final-grade requirement of **5 (45%)**. Anni compares the unrounded current average with **4.50**. This is provisional progress, not confirmation of a final grade or of individual exam and submission requirements. These may be specified in each course's syllabus. [Course descriptions](https://www.flensborg.is/is/namid/nam-og-namshaettir/afangalysingar) are linked in the app. Sources checked 18 October 2026.

Your downloaded data, personal tasks, and checkmarks are saved in SQLite after changes and restored on startup. You can use your plan before connecting, offline, or after a session expires. Refresh failures keep the previous data and appear above the planner, with details in Settings. A successful empty response replaces the old list. Each INNA account keeps its own personal plan on the device.

> **Disclaimer:** This project is not affiliated with, endorsed by, or supported by INNA or InfoMentor. It uses undocumented/private INNA web endpoints that may change without notice.

## Platforms

- **Windows 10/11 x64** — primary development target
- **Raspberry Pi OS 64-bit ARM64** — primary deployment target (1 GB RAM)

## Windows installer

Download the Windows setup executable from the [latest release](https://github.com/fannsonetti/anni/releases/latest), then run it. Setup installs Anni and its INNA sign-in helper for your Windows account, adds Start menu shortcuts for the planner and desktop widgets, offers an optional desktop shortcut, and enables Windows startup by default. No administrator account is required. Reopen **Anni** from a shortcut to change widget settings or quit the app.

### Automatic updates

Installed Windows copies check the latest published GitHub release whenever Anni launches, including at Windows sign-in. A compact mint-and-charcoal startup window shows an animated checking bar, actual download bytes and percentage, checksum verification, and installation. When a newer stable version is available, Anni downloads the matching Windows installer, verifies its SHA-256 checksum, closes the running app, and installs the update without a wizard. A temporary launcher keeps the progress window visible during installation and reopens Anni in the same planner, desktop, or fullscreen mode. Saved schoolwork, sign-in, and preferences stay in place. An automatic update does not re-enable startup after you turn it off.

Offline connections, unavailable releases, and download/verification failures open the existing version instead. **Skip for now** opens it immediately. Failed installs offer **Launch Anni**; the next normal launch checks again. Source builds, mock mode, and Linux do not install Windows updates. Install version **0.2.0** or later once to get this updater; older versions need that initial manual upgrade.

Future releases must use increasing versions, a matching `v<version>` tag, and the `Anni-<version>-windows-x64-setup.exe` / `.exe.sha256` assets produced by the existing release workflow. Updates come from `fannsonetti/anni`; drafts, prereleases, older versions, and missing or invalid assets are skipped. The checks use GitHub's public [latest-release API](https://docs.github.com/en/rest/releases/releases#get-the-latest-release), and the installer uses Inno Setup's [silent-install options](https://jrsoftware.org/ishelp/topic_setupcmdline.htm). Merely editing or building the repository does not publish a release.

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

The script builds both release executables, verifies the Microsoft bootstrapper's Authenticode signature (reusing a verified cached copy when available), and writes the installer and SHA-256 checksum to `dist/`. Use `-SkipBuild` to package existing release binaries, or `-IsccPath` to select a compiler. GitHub Actions builds and tests the installer on pushes and pull requests. Pushing a `v*` tag also publishes the installer as a release; keep the tag version aligned with `crates/inna-app/Cargo.toml`.

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

Choose **Settings → Connect to INNA**, then sign in using Google or your usual INNA method in Anni's separate sign-in window. No browser extension or developer-mode setup is needed.

The sign-in window uses a persistent, non-incognito browser profile under `%LOCALAPPDATA%/inna-client/sign-in` (the equivalent local data directory on Linux). Existing Anni-owned `inna-auth.exe.WebView2` profiles are reused when present so remembered accounts are not discarded. Both locations stay in place during normal updates. The helper closes its native WebView and releases the profile before exiting, allowing browser-managed cookies and site storage to persist. Anni does not read your Google password or automate account selection. Google/INNA may still require reauthentication, account selection, or verification; Google can also reject embedded browser sign-in. Existing valid INNA sessions are reused on startup without opening any sign-in window.

**Disconnect INNA** removes the app's saved INNA session while keeping the remembered browser login. **Forget sign-in or change account → Forget remembered sign-in** also clears only Anni's sign-in profiles, including the Google login remembered there. Downloaded schoolwork and your personal plan are kept; your normal Chrome/Edge profiles are unaffected. Close any Anni sign-in windows first. Login data is never sent to the bug-report server.

**Other sign-in options → Connect with your phone** remains available for devices without an interactive desktop. It uses a local relay on the same Wi-Fi network. External providers can leave the relay, so Google sign-in may not complete in that mode. The experimental Anni Connect extension route from 0.2.2 has been removed; an already installed Anni Connect extension can be removed through the browser's extension manager.

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

**Settings → Desktop** offers seven everyday themes: **Minimal**, **Ocean**, **Sand**, **Forest**, **Lavender**, **Sunset**, and **High contrast**. Each theme coordinates text, secondary text, accent, and background colors and uses a digital clock. High contrast also increases background opacity and text size for readability. Saved Arc and Iron Man preferences migrate to Ocean and Sand, keeping layout and content choices. Choose the theme accent, one of four preset accents, or your own color with the custom color picker. Digital, digital with a ring, and analog clocks are available with date and seconds switches. You can also change widget visibility, width, height, text size, vertical position, row spacing, corner rounding, background opacity, and task course-code/deadline visibility. Appearance changes save automatically. **Reset appearance** restores defaults.

Sync reads the enrolled course list and visits each course's **Verkefni og próf** endpoint, in addition to global projects, global Heimavinna, course Heimavinna, and course homework calendars. It reads INNA's `text` / `date` and `handInFullDate` fields, excludes teaching-goal-only rows, empty HTML, generic Homework/Heimavinna headings, and evaluations from the homework queue, and combines duplicate homework. Actual instructions accompanying a generic homework heading and dated assignments are retained. Calendar checks cover the previous 90 days and next 120 days; the regular Heimavinna and course assignment lists remain unrestricted. Local checkmarks survive refreshes, restarts, and matching homework moving between views. A failed source retains its saved rows and reports a refresh warning; a successful empty response clears that source.

Closing the main window or choosing **Quit Anni** exits Anni and all widgets. Choose **Show desktop widgets** in Settings to hide the planner and keep widgets running. Launching Anni again from its shortcut reopens the existing planner. Use **Settings → Desktop** to change appearance, swap sides, or disable widgets. The hide action is disabled when no widgets are selected.

### Bug logging and the storage computer

Every normal copy records anonymous diagnostic events in `%LOCALAPPDATA%/inna-client/diagnostics.json`. The latest 500 events survive restarts; repeated adjacent events are coalesced for five minutes. Mock mode never records or sends reports. Reports include an anonymous installation ID, app version, platform, error categories, hourly refresh counts, unmapped subject-label counts, invalid lesson-time counts, and the number of non-lesson calendar entries. They exclude names, grades, full schedules, homework content, credentials, raw responses, and raw error messages. Unknown subject codes display their original code instead of “Course.” All-day events and recognized school breaks remain visible as school-calendar notes in Timetable, without becoming courses, counted classes, or the next lesson.

After a 15-second startup delay, then every hour while Anni is running, clients POST queued batches to `http://85.220.66.209:3000/anni/diagnostics`. An unavailable server or a failed delivery keeps the queue for the next attempt. A successful durable write returns HTTP 204; acknowledged events are no longer queued for delivery. Recent local history is retained up to the 500-event limit. **Settings → Bug reports** shows delivery status, lets you copy the diagnostic log, and allows automatic sharing to be turned off. This endpoint uses HTTP, so only the restricted diagnostic schema is sent.

On the intended admin PC, enable **Use this computer to store everyone's bug reports**, or launch `anni --diagnostics-server`. Anni listens on TCP port 3000 while the planner or widgets run and saves received JSON batches under `%LOCALAPPDATA%/inna-client/received-reports`. A PC with `85.220.66.209` directly assigned to a local interface enables storage automatically. When that IP belongs to a router, select the storage PC explicitly: computers behind the same router share a public IP and cannot identify the intended admin machine from that IP alone. Forward the router's TCP port 3000 to the storage PC and allow Anni through Windows Firewall. The app does not change router or firewall rules. The admin PC stores its own reports directly, without requiring router loopback support.

The receiver exposes `GET /anni/health`, accepts only the diagnostic schema with batches of at most 40 events and a 64 KiB request limit, rejects invalid device IDs, and bounds stored files to 2,000 batches / 30 days. Retried batches overwrite the same file. It accepts anonymous reports without a login and offers no remote file-reading endpoint. Reports are files on the storage PC; there is no web admin dashboard. Keep port 3000 available for Anni; Settings reports bind failures.

Enable **Start Anni and check for updates when I sign in to Windows** to register the current executable for your Windows account. Setup enables this by default; the checkbox turns it off or on. Keep the executable at that location; after moving it, turn this setting off and on from the new location. Installed copies show the update window first, then launch desktop mode without leaving the planner open. This option is disabled in mock mode. Desktop preferences are stored separately from account data in `%LOCALAPPDATA%/inna-client/desktop.json`.

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

anni is a separate, unofficial way to view INNA. INNA remains the name of the service and data source. A mint lowercase-a monogram appears in the sidebar, compact header, and native window icon. The same mark is embedded in the Windows executable and installer, so Explorer, Start menu shortcuts, the taskbar, and installed-app listings can display it. The SVG, PNG, and nine-size ICO (16–256 pixels) are in `crates/inna-app/assets/`; `scripts/generate-icons.cjs` regenerates the raster assets from the SVG using Node and `sharp`. The Windows build embeds the committed ICO using the Windows SDK resource compiler; other platforms do not need it.

The desktop sidebar groups daily planning separately from school information, with short descriptions and a clear current-page indicator. Short windows keep the same sidebar with a scrollable menu. Smaller windows show **Today**, **My work**, **Timetable**, and a **More** menu containing every page. Settings stays directly accessible from the desktop sidebar and the compact menu. Back and forward buttons (also **Alt+Left / Alt+Right**) return through page links and announcement details. Clicking the anni logo opens Today. An always-visible page label shows your current location, and the add-task action is available on every page.

The executable is now `anni`; the internal `inna-*` crate names and existing `inna-client` data directory stay compatible with saved sessions and planner data.

## Features

- **Always dark** — fixed dark appearance, including native window chrome, inputs, menus, and every screen; does not follow the operating system theme
- **Today** — personalized dashboard, next class (including the next school day), room, urgent work, and today's schedule; clickable seven-day workload chart opens the deadlines for a selected day
- **My work** — combined deadline queue, local checkmarks, editable personal tasks, quick due dates, undo for the latest deletion, filters and search
- **Focus** — a calmer timer with duration/task settings alongside it, 25/50/90-minute study presets, 5/10/15-minute breaks, custom durations from 1–240 minutes, pause/resume, progress ring, and saved session history with today's study minutes; a running or paused timer is accessible across pages
- **Timetable** — a time-aligned week grid on wider windows, a day agenda with break lengths, course colors, room/teacher details, current/next lesson indicators, and separate lanes for overlapping lessons; click a class to open its day. Smaller windows use the day agenda, and weekends appear in the week grid when lessons are scheduled
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
# Preview the animated updater (no network, installer, or saved-data changes):
cargo run -p inna-app --offline -- --preview-update
# Read-only live connection and homework-source check (uses your saved sign-in):
cargo run -p inna-app --offline -- --test-session
```

Mock mode uses sample deadlines relative to today and does not read or write your real planner. Changes in mock mode are temporary. Automated checks cover deadline formats, overdue sorting, completed work, account isolation, weekend lesson selection, SQLite persistence, and refresh failure handling. Live INNA behavior still depends on its private endpoints and your authenticated session.

Focus timers continue while you navigate within the app. Closing the app discards a running or paused session; only completed study sessions are saved. Breaks do not count toward study minutes. Switching INNA accounts stops the active timer and keeps completed session history with the corresponding account. Task checkmarks and announcement filters are local views and do not change submission or read status in INNA.
