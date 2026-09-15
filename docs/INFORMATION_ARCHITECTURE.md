# Information Architecture: TuneX V1

> Companion to `DESIGN_BRIEF.md` in this folder. Sources: `docs/SPEC.md` §§4/13–15/18–19, `docs/project/REQUIREMENTS.md` (R-001–R-018), `docs/project/ARCHITECTURE.md`, `docs/mockup.png` (canonical layout reference — content adapted to local library per the brief's adaptation map). Desktop app — "pages" are QML views in one window; see View Routing Strategy for the URL adaptation note.

## Site Map

View ids in `monospace` (QML view names, S1–S4 scope). Max depth: 2.

- Shell `TuneXWindow` (rail + top bar + content + persistent player: right panel at ≥1280px, bottom mini-player bar below that)
  - Home `HomeView`
  - Search `SearchView` (results grouped in place)
  - Library
    - Artists `ArtistListView` → Artist detail `ArtistDetailView`
    - Albums `AlbumListView` → Album detail `AlbumDetailView`
    - Songs `SongListView`
    - Genres `GenreListView` (list → filtered songs; Unknown stays Unknown)
    - Composers `ComposerListView` (list → filtered songs; Unknown stays Unknown)
    - Folders `FolderListView` (browse tracks by parent folder; add/remove roots live in Settings → Library)
  - Playlists `PlaylistListView` → Playlist detail `PlaylistDetailView`
  - Favorites `FavoritesView` (playlist-like smart view over local favorites)
  - Queue `QueuePanel` (right drawer overlay, not a nav destination)
  - Now Playing `NowPlayingView` (expanded overlay surface, same player state)
  - Settings `SettingsView`
    - Library `SettingsLibraryView`
    - Playback `SettingsPlaybackView`
    - Notifications `SettingsNotificationsView`
    - Appearance `SettingsAppearanceView` (theme, accent, blur/motion toggles, Now Playing view)
    - Shortcuts `SettingsShortcutsView` (V1: view bindings; customization deferred)

## Navigation Model

- **Primary navigation (left rail, max 9 items + playlists, labels per the mockup):** Home · Search · Your Library section (Artists · Albums · Songs · Genres · Composers · Folders) · Playlists section (+ New → Favorites · user playlists, scrollable). Selected item gets accent indicator bar + selected surface; everything else muted. Rail never exceeds two visual groups plus playlists.
- **Secondary navigation:** in-view tabs/segmented headers only where content genuinely splits — Search results groups (Songs/Albums/Artists), Playlist detail (fixed header + rows, no tabs in V1), Settings sections (left list + detail pane). No secondary nav inside Artist/Album detail beyond back.
- **Utility navigation (top bar + window):** back/forward (view history), global SearchBar (focus via `/` or Ctrl+K, Esc restores), settings gear, window controls. Queue toggle and Now Playing expand live in the persistent player, not the top bar.
- **Persistent player:** right Now Playing panel at ≥1280px (as in the mockup); bottom mini-player bar on every view below that width. Expanded Now Playing is a full overlay at any width (same session playlist). Player state is global — navigation never interrupts playback.
- **Window-size adaptation (no mobile):** ≥1280px three-column; 1024–1279px right panel becomes overlay drawer; 800–1023px rail collapses to 64px icon strip; <960×640 not allowed (min window). Meaning never changes across sizes, only placement.

## Content Hierarchy

### HomeView (the 80% view alongside Search + Queue)
1. Greeting hero — time-of-day line + one primary action ("Play Something" / "Continue"). Why first: zero-decision start, biggest first-run drop-off killer.
2. Genre chip strip (library-backed filter shortcuts, "More" overflow). Why second: fastest narrowing gesture, mirrors the mockup's strip with real data.
3. Recently Played rail (artwork cards). Why third: recognition — collectors re-find by cover.
4. Recently Added rail. Why fourth: rewards the scan habit, surfaces new files.
5. Library shortcuts (Artists/Albums/Songs/Folders entry cards). Why last: escape hatches, not the daily path.

### SearchView
1. Grouped live results — Songs, then Albums, then Artists. Why: most keystrokes name a track. Tab / Shift+Tab cycles those groups.
2. Recent searches (local only, cap 10). 3. Empty states: no-query hint, no-results with scope note ("searched your library"). Album and artist hits open the library landing pages.

### ArtistDetailView / AlbumDetailView
1. Header: art, name, one primary Play, key meta (albums count / year / duration). 2. Track list (virtualized rows). 3. More-by context (other albums by artist) on album view. Artist card activation opens this page; Play is the header primary (it does not enqueue from the grid).

### SongListView / FolderListView / GenreListView
1. Page banner (unique per library page) + Play all (primary, leading) and Sort By (secondary, trailing; Songs/Albums/Artists). Rail items switch Tracks / Albums / Artists / Genres / Composers / Folders — no in-content tab chips. 2. Virtualized TrackList or artwork grid. 3. Folder view: directory list → tracks in that folder (library-root add/remove, rescan, and watcher status live in Settings → Library, not this view). Missing-file states stay on the track rows.

### PlaylistDetailView
1. Header: mosaic art, name, play/shuffle, edit actions. 2. Ordered TrackList with remove/reorder affordances. 3. Missing-track rows preserved as dimmed (never silently dropped).

### QueuePanel (Now Playing)
1. Session playlist header "Now Playing (n)". 2. Ordered rows with remove/reorder + "Play next" semantics. 3. Drop songs onto the list (including empty). 4. Clear + shuffle toggles. The expanded overlay is the same session playlist, larger.

### NowPlayingView
1. Large artwork well (Artwork / Spectrum / Waveform / Visualizer; lyrics pane still replaces it when the lyrics toggle is on). 2. Title/artist (+ favorite). 3. Progress + times. 4. Transport + shuffle/repeat + volume + lyrics toggle. Lyrics are local sidecar `.lrc` or embedded unsynced tags — not a tab farm and not a provider.

### SettingsView
1. Library (music folders add/remove/rescan, watcher status, named profiles, opt-in MusicBrainz default off). 2. Playback (gapless note, volume, shuffle/repeat, ReplayGain, equalizer, crossfade, output device, close-to-tray). 3. Notifications (desktop toasts; track-change only when unfocused). 4. Appearance (dark locked V1, accent, blur, motion, Now Playing view). 5. Shortcuts (complete reference list of real bindings; not a rebind UI).

## User Flows

### First run → music in <30s
1. Launch → empty-state Home ("No music yet") with single Add-folder action.
2. Action opens Settings → Library; native folder dialog → root added → scan starts with live counts + %.
3. First artwork lands → hero action becomes "Play Something".
4. Press play → persistent player appears (right panel at full width, mini-player bar below), playback starts; browsing continues uninterrupted.

### Search → play
1. `/` focuses SearchBar from anywhere → type 2+ chars.
2. Grouped results render debounced (~150ms); arrows move, Enter plays top song.
3. Esc clears query → previous view + scroll position restored.

### Build an evening queue (queue-as-instrument)
1. From any library/search/playlist row: ⋯ → "Play next" / "Add to Now Playing". Now Playing rows use right-click / Menu / Shift+F10 (no ⋯). Ctrl/Shift select several TrackRows, then Play selected / Add to Now Playing / New playlist.
2. Open QueuePanel → reorder via drag/keyboard; playing row marked; drop songs onto the list.
3. Gapless handoff between consecutive albums (about-to-finish preload, no user action).

### Curate a playlist
1. Rail + → name playlist → detail view created.
2. Add from any row/album ("Add to playlist" picker) → rows append.
3. Reorder/remove in detail; missing files stay dimmed; playlist survives restart.

### Offline Sunday (all flows, network off)
1. Disable network → badge-free, identical app (no warnings, no degraded copy).
2. Rescan, search, queue, playlists, MPRIS keys all behave as online — verified per R-017.

### Missing-file recovery
1. File deleted externally → row dims with "File missing".
2. Row offers Reveal-in-files (if parent exists) and library rescan entry.
3. Rename detected → identity reconciles, playlist/history links preserved (no duplicate).

## Naming Conventions

| Concept | Label in UI | Notes |
|---------|-------------|-------|
| Track (domain) | Track / Tracks | Code says Track (D-008). View title "Tracks". |
| Queue (domain) | Now Playing | Right-rail header "Now Playing"; the overlay is the expanded player of the same session playlist (`playback_queue`). Settings/docs may still say queue. |
| Favorites | Favorites | Heart toggle; a local collection, never a service ("Liked Songs" wording banned). |
| Library roots | Music folders | Settings label; "Library Root" never shown. |
| Now Playing overlay | Now Playing | Expanded overlay title; same state as the rail. |
| Playlists | Playlists | User collections; "Mix/Weekly/Discover" algorithmic names banned in V1. |
| Genres | Genres | Filter list → songs; untagged group is Unknown. |
| Composers | Composers | Same shape as Genres; untagged group is Unknown. |
| Missing track | File missing | Exact copy; never "Unavailable" (implies service). |

## Component Reuse Map

| Component | Used on | Behavior differences |
|-----------|---------|---------------------|
| NavigationRail | All views (shell) | Icon-strip variant <1024px; same selection model |
| TopBar + SearchBar | All views (shell) | Search text persists per session; Esc scope-aware |
| TrackList / TrackRow | Tracks, Artist/Album detail, Playlist detail, Search tracks, Favorites, Folders, Now Playing | Playlist + Now Playing add drag-reorder + remove + drop; library lists Sort By only; missing rows dimmed everywhere |
| AlbumCard / ArtistCard grid | Home rails, Artists, Albums, Search groups | Fixed card contract; column count fluid by width |
| HeroCard | Home only | Greeting variant: empty-state vs. continue-listening |
| MiniPlayer | Narrow widths (panel hidden) | Bottom transport bar; hidden pre-first-play (empty state instead) |
| NowPlayingView + QueuePanel | Overlay everywhere | Same state object as the persistent player; drawer vs. full overlay by width |
| GlassPanel dialog/menu | Menus, dialogs, Now Playing backdrop | Never used for plain list rows |
| EmptyState | Home, Search, Playlists, Settings Library | Per-context action (add folder in Settings / clear search / create playlist) |
| Settings section frame | All SettingsView sections | Identical list-detail scaffold |

## Content Growth Plan

Libraries grow to 50k+ tracks; the IA absorbs growth structurally, not with more pages:

- All long lists virtualized (TrackList) with stable scroll position across navigation.
- Grids paginate by window (no infinite-scroll spinners blocking); search is the primary retrieval path at scale, always one keypress away.
- Recently Played/Added rails capped (e.g. 20) from local history; full history stays queryable via Tracks + sort.
- Playlists are user-bounded; playlist list scrolls inside the rail section with its own overflow.
- No new top-level destinations may be added for scale — growth goes into search ranking, filters, and sort, not nav items (guards R1 scope-creep risk).

## Progressive Disclosure Map

Every secondary layer below has exactly one explicit affordance and is keyboard/screen-reader reachable:

| Hidden layer | Revealed by | Context |
|--------------|-------------|---------|
| Row actions (play, play-next, add to Now Playing, add-to-playlist, remove) | Hover/focus on row; always-visible ⋯ on library/search/playlist lists; right-click / Menu / Shift+F10 everywhere | All TrackLists; Now Playing omits ⋯ |
| Playlist / Now Playing drop | Drag a TrackRow (12px); chip follows the pointer; drop target washes + accent edge + caption | Now Playing, playlist detail, playlist sidebar |
| Now Playing list | Queue toggle in persistent player | Global (drawer/overlay by width); same session playlist as the overlay |
| Now Playing expanded | Mini-player click or shortcut | Global overlay, same state |
| Artwork / Spectrum / Waveform / Visualizer | Artwork well context menu View; Settings → Appearance | QueuePanel + overlay; lyrics still wins |
| Local lyrics overlay | Lyrics toggle on Now Playing | Replaces artwork; sidecar `.lrc` or embedded tags |
| Full result lists | "See all" per search group | SearchView |
| Extra genre chips | "More" overflow | HomeView strip |
| Advanced settings | Per-section secondary level | SettingsView |
| Destructive/secondary ops | Context menus (⋯), never toolbars | Rows, playlists, queue |

## View Routing Strategy

Adaptation note: TuneX is a single-window QML app — there are no URLs. Navigation is a state model instead:

- Pattern: `view://{section}/{subview}/{id}` as internal route keys only (e.g. `view://library/album/42`), backing the rail selection, back/forward stack (depth ≥20), and Esc behavior.
- Dynamic segments: entity ids from SQLite rowids; missing entities resolve to a "no longer in library" state, never a crash.
- Filtering/sorting are view-local state (per-view memory, not global query params) and reset only on explicit clear.
- External integrations (MPRIS, media keys, file-manager "Open with") address *player state*, never routes: they control play/pause/next/previous and enqueue files without disturbing the current view.
