# Themes

The TUI ships with three palettes:

- **`dark`** — fuchsia/purple accent on a dark background
- **`light`** — blue/indigo accent on a light background
- **`solarized`** — Solarized Dark

By default the TUI queries the terminal background via OSC 11 and picks `dark` or `light` automatically. If you'd rather pin one, drop a config:

```toml
# ~/.rssdude/config.toml
[ui]
theme = "dark"     # or "light" | "solarized" | "auto"
```

`"auto"` is the default and triggers the OSC 11 detection.

## Color depth

`rssdude` always uses truecolor (24-bit). Terminals from the last decade all support it; if you're on a TTY that doesn't, the colors will degrade gracefully but won't look great. There's no manual fallback knob — fix the terminal, not the config.

## Visual cues

The visual layer is intentionally restrained. The signals to know:

| Marker | Meaning |
|---|---|
| `●` | Unread item |
| `★` | Starred |
| `▸` / `▾` | Folder collapsed / expanded |
| `┃` (left gutter) | Currently selected row |
| `▪` | Selected in visual mode |
| `!` (prefix on a feed) | Recent fetch errors |
| `├──` / `╰──` | Sidebar tree connectors |
| `•` (separator) | Metadata separator |

## Custom themes

v3 ships presets only. Custom palettes aren't supported yet — if you want one, file an issue describing the palette and the use case.
