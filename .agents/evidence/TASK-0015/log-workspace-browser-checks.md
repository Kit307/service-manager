# Log workspace browser verification — 2026-10-21

Rebuilt actual React `src/main.tsx` and production styles into the offline synthetic fixture. Added nine synthetic service definitions only in the fixture, for twelve available services. Read-only mock invocation blocks lifecycle, configuration, and Clear output mutations. No server, Manager, credentials, or real services were used.

- Initial overview hides Log workspace. An overview card opens its service as one tab; sidebar selection appends another tab while workspace is open.
- Opened eleven tabs without a cap and added panes beyond two.
- Real pointer input reordered Web app before API server, moved API server into pane two, and dropped Web app onto the new-pane target to create pane four. Stored tab order confirms every operation.
- The API search `health` remains after tab switching, pointer moves, hiding to AI Connections, and reentering Log workspace. Reentry restores the same layout and two matching log lines.
- At 1600×1000 and 900×600, document dimensions match the viewport. At minimum size, workspace grid clientWidth and scrollWidth are both 680px. Pane content scrolls internally.
- Browser runtime error list and mutation invocation list remain empty.

Visual evidence: `log-workspace-wide.png`, `log-workspace-900.png`. Measurements and complete before/after tab layout: `log-workspace-browser-checks.json`.

Final source recheck: rebuilt after the drag-gap regression fix. Real pointer crossed the visible grid gap; `.dragging-log` and the new-pane drop target remained visible. Dropping then created pane five with the moved tab. Final screenshots were regenerated from this build; minimum viewport still has no horizontal overflow and no runtime errors. `log-workspace-final-source-checks.json` records build/test timestamps and per-file SHA256, with aggregate source SHA256 `46e72f29c7b55ccf2e93f2efa7dab136541deca64718070645d60ee1487781b1`.

This verifies browser interaction and rendering; native desktop material is covered by separate native fixture evidence. Stream contents are synthetic, so this does not replace backend lifecycle/log tests.
