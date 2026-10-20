# App icon — 2026-10-20

Created a transparent indigo-violet service-list/play icon with the built-in image generation tool. Source and full generation prompt are recorded in `docs/app-icon.md`. Tauri's installed icon generator produced the PNG and macOS ICNS assets. Bundle configuration now explicitly includes both.

Verified source dimensions/alpha with sips; visually checked the generated 32px rendition. Production Tauri build passed, including TypeScript/Vite; app bundle is 15.50 MiB. Info.plist reports v0.2.3 and its referenced ICNS exactly matches the new source ICNS. New DMG checksum verification and read-only mounted checks passed for version, icon bytes, executable permission, and Applications shortcut. Prior releases were preserved.

No service/runtime logic changed. The user's existing app was not restarted or replaced. Dock display after installation was not checked; this development release remains unsigned/unnotarized.
