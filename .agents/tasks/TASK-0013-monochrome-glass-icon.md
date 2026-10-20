# TASK-0013 - Monochrome Liquid Glass app icon

## Objective
Replace the app icon with a black-and-white Liquid Glass design, as requested.

## Scope
Generate a single app icon using built-in imagegen, retain its transparent corners, and convert it into PNG/ICNS bundle assets. Package it with v0.5.1. No installed-app replacement or cache reset.

## Verification
Inspect the generated image, validate source dimensions and alpha, convert all macOS iconset sizes, and compare the mounted DMG icon with the new source ICNS.

## Completion
Complete. Source PNG alpha/dimensions and ICNS chunk structure validated; release build passed and the read-only mounted v0.5.1 DMG icon matched the new ICNS byte-for-byte. See TASK-0012/artifact.json for the final release receipt.

## Generation evidence
Built-in imagegen produced the monochrome terminal glyph on a smoked-glass tile with transparent corners. Source: `src-tauri/icons/app-icon-source.png`; runtime PNG: `src-tauri/icons/icon.png` (1024 × 1024, alpha present); macOS bundle: `src-tauri/icons/icon.icns`. Iconset conversion includes 16 through 1024 pixel representations; ICNS chunk lengths and header validated. The exact generation prompt is recorded in the versioned output guide.
