# Log header alignment — final browser verification

Actual-source offline fixture rebuilt 2026-10-21T08:52:32Z; tested 08:52:43Z. Aggregate source SHA256: `56ba6ff4a3a0f188be6f115494e384dfd048f2ce9ef9a89fdbadc1005abe1d0b`. Individual source hashes and measurements are in `log-header-browser-checks.json`.

- At 1720×1000, narrow pane header is 360px wide. Tab, plus sign, select and close button vertical centers are all 126.5px: zero alignment difference.
- Eight tabs overflow only the inner tablist (1285px scrollWidth, 161px clientWidth). Real horizontal wheel changes its scrollLeft from 0 to 777.5px while Add/select/Close button positions remain fixed and inside the header.
- At 900×600, the first header is 674px wide; the same controls remain on one row with matching vertical centers. Document dimensions match the viewport at both sizes.
- Real pointer dragging onto the new sibling header still moves the tab into the target pane.
- Reduced transparency makes header and pane opaque `rgb(233,237,242)`.
- No browser runtime errors. Browser fixture closed; no services started or mutated.

Screenshots: `log-header-wide.png`, `log-header-900.png`. These final header screenshots supersede the earlier header captures that showed a scrollbar-induced vertical offset. Native desktop material is covered separately.
