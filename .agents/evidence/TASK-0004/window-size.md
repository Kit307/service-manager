# Start maximized — 0.2.1

The user requested avoiding a reduced window. Clarification offered maximized startup versus locked resizing; no answer arrived during the optional clarification interval. The implementation uses the reversible first option and states that assumption explicitly.

- Current native window was expanded using the exposed zoom action; accessibility/screenshot confirmed the larger window and removal of the compact-window scrollbar. No service was started or stopped.
- Set the single main-window Tauri `maximized` flag to true. Retained the existing default resizability, normal dimensions and minimum bounds. No renderer/runtime service logic changed.
- Version metadata updated consistently to 0.2.1.
- Checked the installed Tauri configuration schema: maximized is the native startup window flag, independent of fullscreen.
- JSON parsing/assertions and release frontend/native build validate the configuration. No new unit tests for a single native configuration flag.
- Fresh-launch behavior is configured in the new bundle; the already running user's process was expanded without restarting it or modifying its projects.
