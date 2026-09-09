# Frontend rules — TuneX (loaded via `opencode.json` instructions)

## Mandatory skill trio

Load **all three** before any UI work, and follow them plus their references:

- `craft-beautiful-frontend` — design system discipline, quality gates, cognitive psychology (Hick/Miller/Jakob/Fitts).
- `iconography-frontend-ui` — icon decisions, sizing, states, ARIA/WCAG non-text contrast.
- `responsive-design` — window-class strategy, fluid artwork grid.

Web/CSS specifics in those skills translate to QML:

| Skill concept | TuneX/QML equivalent |
|---------------|----------------------|
| CSS custom properties / tokens | `docs/DESIGN.md` frontmatter + Theme singleton |
| Lucide SVG components | SVG icon assets, single 2px rounded line family (16/20/24px, 44px hit areas) |
| Media queries / container queries | Window-width classes (≥1280 / 1024–1279 / 800–1023, min 960×640) + `GridView` flow cells |
| CSS transitions / keyframes | QML `Behavior`s within `docs/DESIGN.md` motion budgets (120–220ms, reduce-motion honored) |
| Focus-visible / ARIA | Qt Accessibility names, 2px `{colors.focus}` ring, keyboard-first flows |

## Mockup law (`AGENTS.md` Frontend Rules §2)

- `docs/mockup.png` defines layout/chrome/components; `docs/DESIGN_BRIEF.md` adaptation map defines content (local data in mockup-shaped components).
- Real components bound to real Rust models only. No lorem-ipsum, no invented screens. Placeholders only where `docs/DESIGN.md` specifies (artwork monogram, "Unknown", EmptyStates).
- `docs/DESIGN.md` frontmatter is normative — never contradict token values.

## Per-change quality gates

Type scale · 4px spacing · AA contrast · one primary action/view · 44px targets (40px dense-list exception) · motion in budget · focus visible · empty/loading/error/success states · no color-only meaning · single icon family · ≤7 primary choices (progressive disclosure).
