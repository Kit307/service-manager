# Service Manager app icon

The v0.2.3 icon uses three service rows and a play triangle on an indigo-violet rounded tile. The surrounding background is transparent. It was generated with the built-in image generation tool and visually checked at 32 pixels.

Source: `src-tauri/icons/app-icon-source.png` (1254 × 1254, RGBA). Tauri generates `icon.png` and `icon.icns`; the bundle explicitly includes both. No runtime behavior changes.

To regenerate the platform files, run `pnpm tauri icon src-tauri/icons/app-icon-source.png --output <temporary-directory>` and copy `icon.png` and `icon.icns` into `src-tauri/icons/`.

## Generation prompt

Create one polished macOS app icon for 'Service Manager', a lightweight developer desktop app that starts and stops local project services. Logo-brand app icon asset, square 1024x1024 composition, not a mockup or presentation. One centered macOS squircle tile with softly rounded corners and generous transparent padding outside the tile. Deep saturated indigo-violet tile, subtle refined lighting and very slight bevel. Center symbol: three clean horizontal white service tracks with circular status nodes on the left; a distinct white right-pointing play triangle integrated at right. Bold geometric silhouette, sparse details, legible at 32px. Strong optical balance. Premium native desktop utility character. No letters, no words, no watermark, no extra objects, no background scene, no surrounding border. The entire area outside the icon tile must be genuinely transparent.

The generated source resolution is 1254 × 1254 despite the requested 1024 × 1024. Tauri handles platform resizing.
