# Glass surface browser checks — 2026-10-21

Actual React `src/main.tsx` bundled with installed esbuild into a disposable offline HTML fixture at `/tmp/glass-preview-061/index.html`. No HTTP server or Manager was started. Synthetic Tauri invocation permits workspace/log/history/diagnostics reads and event no-ops; every mutation rejects. The pastel backdrop exists only in the fixture to expose transparent surfaces. It does not represent native desktop blur.

## Verified

- Real opacity range control, keyboard Home/End: 30% and 100% in light and dark appearance.
- Shell computed background changes from alpha `0.3` to `1`. Text opacity remains `1`; log backgrounds remain alpha `0.96` at both values.
- Clear button background alpha `0.14`, hover alpha `0.28`; selected tabs alpha `0.28`. Rounded rims and inset highlights visible in light/dark screenshots.
- At 900×600, document dimensions are exactly 900×600. Footer is y553–600; opacity control y486.5–541. Settings panel stays inside x205–900/y296–600 and scrolls internally.
- Emulated reduced transparency gives opaque shell `rgb(32,36,43)` and button backdrop filter `none`.
- No browser runtime errors or blocked mutation calls during these checks.
- `pnpm build` and `pnpm lint` passed after adding the surface stylesheet.

## Visual evidence

- `glass-surfaces-light-30.png`
- `glass-surfaces-dark-30.png`
- `glass-overview-900.png` — final responsive overview, 900×600, document matches viewport with no horizontal overflow. Cards are 616px wide and 170.6px high (failed card 223.8px); vertical content scrolls internally.
- `glass-overview-wide.png` — final overview at 1600×1000, three 389.3px columns within 1200px content limit.
- `glass-overview-checks.json` — measured final overview dimensions, no runtime errors.

The dark theme at 30% on an intentionally bright backdrop reduces contrast of muted labels. Raising background opacity restores stronger separation; log text remains sharp. Native Liquid Glass support and actual desktop blur are validated separately by the native fixture.
