# Console redesign browser QA — 2026-10-21

Actual production React/CSS rebuilt into the offline fixture after the default pane minimum changed to 440px. Source SHA256: `6f2cf9119da43f4b89c06eccdbc35b983d1f8959018f1e14507d634dcdc55cb2`; build/test times and per-file hashes are in `log-console-redesign-checks.json`.

## Visual review

The redesigned console has a clear hierarchy: Logs heading, service tabs with one understated underline, then one search/stream/action row. It removes the previous repeated pill controls and duplicate output headings. Light and dark log paper remain sufficiently opaque to keep text sharp against the synthetic backdrop. The empty pane has a deliberate centered instruction rather than a blank container. Default 440px minimum is preferable to 360px: message text has more room and wraps less.

At 1720×1000 the four-pane fixture uses three approximately 485px columns and a second row for the intentionally empty pane. The grid reaches y998; there is no old fixed 650px height cap. At 900×600 panes stack and scroll internally, with document dimensions exactly matching the viewport. A separate fixture-only forced four-column layout checked 360.5px panes: toolbar, tab actions and message wrapping remain usable. This forced layout is not a production default.

## Interaction and accessibility

- Search `health` leaves two matching lines.
- Direct browser keyboard typeahead on the native invisible stream select changes it to `stdout`; the focused wrapper has a solid focus outline.
- Native Add log overlay accepts keyboard typeahead and opens Background worker in the empty pane.
- Follow toggles off and reports `aria-pressed=false`.
- Keyboard Tab reaches Clear output with a solid outline. Browser accessibility tree names Follow/Following, Copy and Clear output correctly. Clear was never activated.
- Real pointer drag onto the redesigned sibling header moves the API tab into the target pane.
- No service/configuration/Clear mutations occurred. Synthetic fixture only; no server or Manager started. Browser task closed after verification.

Screenshots inspected: `log-console-redesign-light-wide.png`, `log-console-redesign-dark-wide.png`, `log-console-redesign-900.png`, `log-console-redesign-forced-360.png`.

Native desktop material is verified separately. This browser pass does not replace backend lifecycle tests or the frontend test suite.
