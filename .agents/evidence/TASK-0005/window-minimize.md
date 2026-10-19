# Window minimization — 2026-10-19

The user clarified that only minimizing to the Dock should be disabled. This supersedes the maximized-startup assumption in v0.2.1.

## Changes

- Disable minimization through the Tauri main-window configuration.
- Replace the macOS native Minimize menu action with a disabled menu item, since disabling the titlebar button alone leaves the native menu action available.
- Restore normal startup dimensions (1200 × 820 logical pixels); resizing and zoom remain enabled.
- Version the deliverable as 0.2.2.

## Verification

- Built and launched an isolated debug bundle with identifier `dev.servicemanager.windowcheck`, using the same window configuration and native menu code.
- Native accessibility inspection: no minimize titlebar button; fullscreen/zoom button present.
- Window menu: Minimize disabled; Minimize All absent; Zoom and Move & Resize present.
- Pressed Cmd+M with the menu closed: the main window remained visible with unchanged accessibility state.
- The empty test app quit through the existing controlled confirmation flow. The user's running Service Manager was left open.
- Configuration assertions passed: minimizable false, maximized false by default, resizable true by default, package/config/Cargo versions all 0.2.2.
- Rustfmt and Clippy (`--all-targets -- -D warnings`) passed.
- Production `pnpm tauri build --bundles app` passed, including TypeScript and Vite; bundle size 13.48 MiB.
- Packaged the new v0.2.2 ZIP without replacing previous releases; verified Info.plist version, ZIP integrity, and executable permission.

No service execution, authentication, or project configuration behavior changed. Native UI verification was on macOS only; this remains a local unsigned/unnotarized development release.
