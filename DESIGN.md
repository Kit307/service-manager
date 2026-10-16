# Service Manager Design System

Developers keep this window beside their editor, moving between project controls and output during the workday and evening. Follow the system appearance so that the workspace does not force a light level different from the rest of the machine.

## Layout
A 232px sidebar, a compact service summary/action area, and a vertically resizable log panel filling the remaining workspace. Minimum usable size is 900 × 600. Settings live in a separate service tab; AI connection settings are a dedicated view.

## Visual language
Restrained neutral surfaces, thin separators, 8px control corners, system sans typography, and monospace commands/logs. Indigo hue 280 is the small selection/action accent; semantic status colors remain distinct. No decorative cards, gradients, illustrations, or page entrance animations.

## Tokens
Tokens use OKLCH in the stylesheet. Light background is neutral white; dark background is neutral charcoal. Text contrast must be checked in both themes. Main text is 13–14px; log text is 12px with comfortable line spacing. Use compact, labelled actions and consistent inline SVG utility icons.

## Interactions
Opening never runs commands. Running, Ready and Failed remain distinct text states. Log follow pauses when users scroll upward. Activity identifies the configured caller profile rather than pretending to verify which AI model made a request. Errors stay visible until acted on or dismissed.
