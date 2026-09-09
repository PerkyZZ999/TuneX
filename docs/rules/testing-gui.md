# GUI testing rules — TuneX (loaded via `opencode.json` instructions)

## Tooling

- **Kwin-MCP is the designated GUI automation path.** Load the `kwin-mcp` skill before any desktop-app testing and follow its session workflow and schema guardrails exactly.
- There is **no separate built-in Computer Use tool** in this environment — the `kwinmcp_*` tools (session, screenshot, accessibility tree, mouse/keyboard/touch) are the computer-use surface. Do not wait for or ask about another tool.

## Session discipline

- Default to isolated virtual sessions (`session_start`); never touch the user's live desktop (`session_connect`) unless explicitly asked.
- Launch the built TuneX binary (`session_start(app_command=…)` or `launch_app`); confirm with `list_windows`.
- `isolate_home=true` when reproducibility matters (scan/library tests must not read the user's real music dirs — use fixture libraries).
- Always finish with `session_stop` unless the user wants artifacts kept (`keep_screenshots`/`keep_home` only on request).

## How to verify (TuneX specifics)

1. Inspect before acting: `find_ui_elements` / `wait_for_element` / `accessibility_tree` — never blind-coordinate-click. Re-read UI state after every navigation or dialog.
2. Drive with keyboard-first flows (Space, `/`, arrows, Enter, Esc) plus `playerctl` cross-checks from the shell for MPRIS assertions.
3. Prove visual states with `screenshot` / `screenshot_after_ms` (glass hierarchy, artwork/placeholder, buffering, toasts, empty states).
4. Media keys via `keyboard_key`; seek/volume via app controls; network-off DoD runs by disabling networking for the session host where possible, else assert no-network code paths by review.
5. On failure: `read_app_log` first, then `wayland_info`/`dbus_call` if compositor-level. Record evidence (screenshots + tree snippets) in `docs/project/VALIDATION.md`.
6. M5 gate additionally requires the same pass under an X11 session.

## When GUI testing is required

- Every slice's Definition-of-Done rehearsal (install → scan → browse → search → play → queue → playlist → restart, plus offline run).
- Any QML/layout change that screenshots can judge (glass, grids, Now Playing, dialogs, empty states).
- MPRIS/media-key behavior. Unit-testable logic stays in `cargo test` / `qmltestrunner` — GUI runs prove integration, not algorithms.
