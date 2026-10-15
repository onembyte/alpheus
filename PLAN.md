# Storage detail: plan

What every byte on the disk is, measured in the background while the app stays
fast. Read-only: nothing in this view deletes; removal still goes through cards.

"Load" here means how much space applications and macOS take, not CPU or memory.

## Design

1. **Instant top level** (< 100 ms) from APFS (`diskutil apfs list -plist`):
   macOS (System volume), OS support (Preboot, Recovery, Update, VM), and the
   Data volume. On this Mac: macOS 18.4 GB, Preboot 21.8 GB, Recovery 3.0 GB,
   Data 195 GB.
2. **Categories like System Settings → Storage**, measured step by step:
   Applications (per app bundle), Developer, Documents, iCloud Drive, Mail,
   Messages, Photos, Music, Trash, Other Users, macOS (System volume), and
   System Data. Two deliberate differences from Settings: Developer also
   counts toolchains and project folders (`~/dev`, Homebrew, `~/.cargo`,
   `~/.ollama`), and System Data is broken down instead of being one opaque
   number: `~/Library` per app, `/Library`, `/private/var`, Preboot,
   Recovery, VM, and **Unattributed** = Data in use minus everything measured
   (snapshots, purgeable space, folders macOS won't let Alpheus read).
3. **Engine**: a Rust walker on its own thread at macOS background QoS, so CPU
   and disk I/O are throttled and the reclaim scan keeps its speed. It counts
   allocated blocks, follows no symlinks, counts each hard link once, and stays
   on one volume. Breadth-first: categories, then their children, then deeper.
   It emits a throttled event (at most ~5 per second) with the updated tree.
   One walk at a time; cancellable.
4. **Preview**: the last breakdown is cached in app data and shown at launch
   ("as of 14:02"), then updated live by the new walk. Each row shows its size
   counting up while it's being measured.
5. **UI**: a "Storage detail" section below the overview. A row per category
   with a bar; click to drill in (breadcrumb) while the walk continues; the
   rest of the app stays usable. Rows that match a reclaim card link to it.
6. **Live updates**: the top level (APFS volumes, free space) refreshes with
   the existing 60 s tray tick. A full walk reruns when the window comes to
   the front and the last one is over 15 min old, or on Rescan.
7. **Honest gaps**: folders macOS blocks are counted and shown ("12 folders
   unreadable: grant Full Disk Access"), never silently left out.

## Checklist

One item per step, test first. Stop and ask at each gate.

### Before: finish the open work
- [x] Offload verified on the NAS; local copies removed (with approval).
      WorkSpaces and UniFi (root-owned) removed by Franco with his admin password
- [x] Tests pass for the four new cards (Fusion, Codex, diagnostics, Ollama); commit

### Phase A: engine (Rust, no UI)
- [x] A1 APFS volume reader: parse `diskutil apfs list -plist` into OS / Data
- [x] A2 Walker: allocated size, hard-link dedup, no symlinks, one volume,
      unreadable count, cancel flag
- [x] A3 Progressive tree: breadth-first scheduler, throttled snapshots;
      totals only grow; final total matches `du` within 1%
- [x] A4 Category map, as in System Settings → Storage (design item 2)
- [x] A5 Background QoS (utility class, see Gate A), Tauri commands and events (start, cancel, cached),
      cache file
- [x] **Gate A** (2026-10-15, load average ~8 from other sessions): reclaim scan
      alone 18.7 s median (18.7 / 19.0 / 13.4), during a whole-disk walk 13.8 s
      (14.0 / 13.8 / 13.3): no measurable slowdown. Whole-disk walk at utility QoS
      41 s (35.5 s at default), 1.77M files, 195 unreadable folders. Nothing runs
      after a walk ends: its threads are joined. Rerun: `cargo test --release
      --test session_test gate_a -- --ignored --nocapture`

### Phase B: UI
- [x] B1 `useBreakdown` hook: events into state, never blocks other views
- [x] B2 Storage detail section: category rows, live sizes, measuring state
- [x] B3 Drill-down with breadcrumb while the walk continues. The tree keeps
      3 levels below each walked folder (keeping every folder would hold
      hundreds of thousands of rows in memory); drilling past that walks
      just that folder again, into a staging subtree swapped in when done,
      so the totals above it never shrink
- [x] B4 Links to reclaim cards; Full Disk Access notice
- [x] B5 Refresh on focus; top level on the 60 s tick (only while the window is
      visible); a fresh walk after a reclaim that freed space. Done early, at Franco's
      request (2026-10-15): the walk starts at app launch from the backend,
      before the window loads, and the window picks it up when it opens
- [x] **Gate B** (2026-10-15): release build, two launches, screenshots.
      Window in 1.0-1.6 s; storage detail shows under the reclaim scan's spinner;
      finished view with reclaimable links on System Data (15 GB) and Developer
      (14 GB); 80-100 MB of memory. Whole-disk walk took 291 s and 331 s, but the
      load average was ~170 (seven rustc builds from another session); App Nap
      was ruled out (no change with it off). Under Gate A's normal load: 41 s.
      Found: while measuring, rows restart near zero and System Data carries all
      unmeasured space (193 GB): fixed by the snapshot-first items below.
      Not exercised by hand: clicking into rows (no UI automation here).

### Phase S: snapshot first (Franco, 2026-10-15)
Alpheus opens on the last known state, however old, and corrects it in the
background. Proposed; waiting for Franco's go-ahead on the format.
- [x] S1 One snapshot file with the last reclaim cards and the storage detail
      (3 levels), written after every finished scan and walk; read first at launch
- [x] S2 While refreshing, each row keeps its snapshot value until the live walk
      has measured it, then corrects in place; unmeasured space never shows as
      System Data
- [x] S3 Snapshot cards show at once but act only after the fresh scan (the
      backend already refuses ids the last live scan didn't produce)
- [x] **Gate S** (2026-10-15): release build launched from the snapshot. Window in
      0.7-1.5 s; by 2.1-2.6 s it shows the kept cards ("Last scan … · refreshing ·
      actions unlock when it finishes") and every storage row at its kept size,
      marked updating; the end state shows live cards and "Measured …". The
      snapshot file is 20-85 KB. Found and fixed on the way: breakdown_locate ran
      on the main thread and stalled the window under heavy load (66c9873); every
      walk payload re-rendered the whole app and could build a backlog (8fb4149).
      Late screenshots of a window behind others show its last painted frame:
      bring it to the front before judging it.

### Phase C: release (only when asked)
- [ ] Version bump to **0.9.1** (Franco, 2026-10-15), universal DMG, release
