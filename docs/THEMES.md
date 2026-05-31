# GitForge Theme Format

GitForge themes are JSON files that define the complete color palette for the application.

## Installing Themes

Place `.json` theme files in `~/.config/gitforge/themes/`. GitForge discovers themes at startup.

```
~/.config/gitforge/themes/
├── monokai.json
├── solarized.json
└── nord.json
```

## Theme Structure

```json
{
  "name": "My Theme Name",
  "appearance": "dark",
  "colors": { ... },
  "fonts": { ... }
}
```

### Fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | Yes | Display name shown in theme picker |
| `appearance` | "dark" or "light" | Yes | Determines if this is a dark or light theme |
| `colors` | object | Yes | All color tokens (see below) |
| `fonts` | object | No | Font configuration (has defaults) |

## Color Tokens

All color values are hex strings: `"#rrggbb"`.

### Core Colors

| Token | Description |
|---|---|
| `background` | Main background |
| `surface` | Panel/card backgrounds |
| `surface_high` | Elevated surfaces (hover, active) |
| `border` | Default border color |
| `border_focused` | Border color for focused elements |
| `text` | Primary text color |
| `text_muted` | Secondary/muted text |
| `accent` | Primary accent (brand color) |
| `accent_secondary` | Secondary accent |
| `error` | Error/danger color |
| `warning` | Warning color |
| `success` | Success color |

### Sidebar Colors

| Token | Description |
|---|---|
| `sidebar_background` | Sidebar panel background |
| `sidebar_text` | Sidebar text |
| `sidebar_selected` | Selected item background |
| `sidebar_hover` | Hovered item background |

### Git Reference Colors

| Token | Description |
|---|---|
| `commit_hash` | Commit hash text color |
| `ref_branch` | Branch label pill color |
| `ref_tag` | Tag label pill color |
| `ref_remote` | Remote branch label color |
| `ref_head` | HEAD indicator color |

### Diff Colors

| Token | Description |
|---|---|
| `diff_added` | Added line text color |
| `diff_added_bg` | Added line background |
| `diff_removed` | Removed line text color |
| `diff_removed_bg` | Removed line background |
| `diff_hunk_header` | Hunk header background |

### Graph Lane Colors

Eight colors for commit graph lanes, cycling:

`graph_lane_1` through `graph_lane_8`

### UI Colors

| Token | Description |
|---|---|
| `scroll_bar` | Scrollbar track color |
| `scroll_bar_hover` | Scrollbar thumb on hover |
| `selection` | Selected text color |
| `selection_bg` | Selection highlight background |

### Syntax Highlighting Colors

| Token | Description |
|---|---|
| `syntax_keyword` | Language keywords |
| `syntax_function` | Function names |
| `syntax_string` | String literals |
| `syntax_number` | Numeric literals |
| `syntax_comment` | Comments |
| `syntax_type` | Type names |
| `syntax_variable` | Variable identifiers |
| `syntax_operator` | Operators |
| `syntax_property` | Object properties |
| `syntax_tag` | HTML/XML tags |
| `syntax_attribute` | HTML/XML attributes |
| `syntax_constant` | Constants |
| `syntax_module` | Module names |
| `syntax_punctuation` | Punctuation |

## Font Configuration

```json
"fonts": {
  "ui": "Inter",
  "mono": "JetBrains Mono",
  "ui_size": 13.0,
  "mono_size": 13.0
}
```

| Field | Default | Description |
|---|---|---|
| `ui` | "Inter" | UI font family |
| `mono` | "JetBrains Mono" | Monospace font for code |
| `ui_size` | 13.0 | UI font size (px) |
| `mono_size` | 13.0 | Code font size (px) |

## Minimal Theme Example

All color tokens have defaults, so you can create a theme with only the fields you want to override:

```json
{
  "name": "Minimal Theme",
  "appearance": "dark",
  "colors": {
    "background": "#1a1b26",
    "surface": "#24283b",
    "text": "#c0caf5",
    "accent": "#7aa2f7"
  },
  "fonts": {}
}
```

Missing colors will use the default dark theme values.

## Tips

- Use a color contrast checker to ensure WCAG AA compliance
- Test with both code and prose content
- The graph lane colors should be visually distinct from each other
- `accent` is used for interactive elements — make it stand out
- `diff_added_bg` and `diff_removed_bg` should be subtle tints, not saturated backgrounds
