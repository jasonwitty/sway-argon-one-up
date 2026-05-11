# Adding a New Theme

This guide documents how to add a new theme to the Sway Argon ONE UP desktop. The theme system is template-driven: a single flavor file defines ~35 colors and metadata, and `switch-theme` renders those into every themed app (sway, waybar, foot, mako, swaylock, wofi, wob, GTK, Brave/Chromium).

If you're adding a theme and something doesn't line up, check the **Gotchas** section at the bottom — most of it was learned the hard way.

## TL;DR — The Four Things You Touch

1. `sway-themes/<flavor>` — the color/metadata file (required)
2. `wallpapers/<flavor>.{png,jpg}` — the default wallpaper (required)
3. `gtk-themes/<GtkThemeName>/` — a GTK theme directory (only if you're bundling a new one)
4. `bin/switch-theme` — add `<flavor>` to the hardcoded list in `available_flavors()`

That's it. The installer (`install.sh`) recursively copies `sway-themes/`, `wallpapers/`, and `gtk-themes/`, so no installer edits are needed.

---

## Step 1: Pick the Flavor Name

Use a short lowercase slug with hyphens, e.g. `arc-raiders`, `monokai-light`. The slug is used for:

- The flavor filename: `sway-themes/<flavor>`
- The wallpaper filename: `wallpapers/<flavor>.{png,jpg}`
- The argument to `switch-theme <flavor>`

## Step 2: Create the Flavor File

Create `sway-themes/<flavor>` (no extension). Use an existing flavor as a starting point — `sway-themes/dracula` is a good minimal reference, `sway-themes/arc-raiders` shows an override-heavy example.

### Required metadata

```bash
THEME_NAME="Display Name"      # Shown in `switch-theme` output
FLAVOR="flavor-slug"           # Must match the filename
COLOR_SCHEME="prefer-dark"     # or "prefer-light"
GTK_THEME="GtkThemeName"       # Must match a dir in ~/.themes/ (or bundled gtk-themes/)
ICON_THEME="Papirus-Dark"      # or "Papirus" for light themes
FOLDER_COLOR="teal"            # Papirus folder color — see Step 5
WALLPAPER="$HOME/.wallpapers/<flavor>.jpg"
```

### Optional overrides

```bash
BROWSER_COLOR="1d1d23"         # Override for Brave/Chromium seed color — see Gotchas
```

### Color palette (all required — 20 values)

The templates reference every color below. Hex values are **without** the `#` prefix.

**Accent colors (14):** `ROSEWATER`, `FLAMINGO`, `PINK`, `MAUVE`, `RED`, `MAROON`, `PEACH`, `YELLOW`, `GREEN`, `TEAL`, `SKY`, `SAPPHIRE`, `BLUE`, `LAVENDER`

**Text scale (6, light → dark):** `TEXT`, `SUBTEXT1`, `SUBTEXT0`, `OVERLAY2`, `OVERLAY1`, `OVERLAY0`

**Background scale (6, light → dark):** `SURFACE2`, `SURFACE1`, `SURFACE0`, `BASE`, `MANTLE`, `CRUST`

Variable names come from the Catppuccin palette convention — stick to it even if your theme's native palette uses different names. Map your theme's colors onto these roles by their function, not their name.

Quick reference for which variables drive what:

| Role                          | Variable    |
|-------------------------------|-------------|
| Default window bg             | `BASE`      |
| Panel / titlebar bg           | `MANTLE`    |
| Deepest shadow / inactive     | `CRUST`     |
| Main foreground text          | `TEXT`      |
| Focused border + accent       | `BLUE`      |
| Error / urgent                | `RED`       |
| Success indicators            | `GREEN`     |
| Warning                       | `YELLOW`    |
| Secondary accent              | `MAUVE` / `PEACH` |

Run `grep -h -oE "@@[A-Z0-9_]+@@" sway-themes/templates/*` to see exactly which variables are referenced by at least one template — any unused ones are still expected to be present for future-proofing.

## Step 3: Add the Wallpaper

Drop a single image into `wallpapers/<flavor>.{png,jpg,jpeg,webp}`. `swaybg` handles scaling via `-m fill`, so pick something at or above the panel's native resolution (1920x1200 on the Argon ONE UP). The wallpaper is set automatically on theme switch.

Users can override per-session with `switch-theme --wallpaper-picker` — their override takes precedence over the theme default and is cleared with `--wallpaper-reset`.

## Step 4: Register the Flavor in `bin/switch-theme`

This is the one file edit outside the theme directories. Open `bin/switch-theme` and add your flavor to the hardcoded list on line 12:

```bash
available_flavors() {
    for f in "$THEMES_DIR"/{frappe,mocha,latte,macchiato,dracula,nord,gruvbox,monokai-dark,monokai-light,arc-raiders,YOUR-FLAVOR}; do
        [ -f "$f" ] && basename "$f"
    done
}
```

Order matters for the `--picker` wofi list. Place the flavor wherever you want it to appear.

## Step 5: Pick a GTK Theme

The `GTK_THEME` value in your flavor file must match a directory name that lives in `~/.themes/` (which the installer populates from `gtk-themes/`). Two options:

### Option A: Reuse an existing GTK theme

The repo currently bundles:

| GTK Theme Name                              | Best fit              |
|---------------------------------------------|-----------------------|
| `catppuccin-{frappe,mocha,latte,macchiato}-blue-standard+default` | Catppuccin variants |
| `Dracula`                                   | Purple/pink dark themes |
| `Nordic`                                    | Cool muted dark themes |
| `Gruvbox-Dark`                              | Warm earthy dark themes |
| `Monokai`                                   | High-contrast vivid themes |
| `ArcRaiders`                                | Very dark with neon accents |
| `Adwaita`                                   | Fallback (light) |

Just set `GTK_THEME="<name>"` in your flavor file. No other work needed.

### Option B: Bundle a new GTK theme

1. Drop the theme directory into `gtk-themes/<GtkThemeName>/`. It must contain at minimum `index.theme`, `gtk-3.0/gtk.css`, and `gtk-4.0/gtk.css`.
2. **Include a license file** — all bundled themes must have one (LGPL, GPL, MIT, CC-BY-SA, etc.). If you can't find one in the upstream repo, you can't bundle it.
3. Set `GTK_THEME="<GtkThemeName>"` in your flavor file.

The installer copies `gtk-themes/*` into `~/.themes/` automatically (`install.sh:411`).

### FOLDER_COLOR options

Papirus supports these folder tints via `papirus-folders -C`:

`black, blue, bluegrey, breeze, brown, cyan, deeporange, green, grey, indigo, magenta, nordic, orange, pink, red, teal, violet, yellow, white, yaru, paleorange, palebrown, warmbrown`

Pick one that complements your theme's BASE color.

## Step 6: Test

From the running desktop:

```bash
switch-theme <flavor>
```

Verify:

- [ ] Sway borders picked up the new BLUE/BASE
- [ ] Waybar recolored (restart is automatic)
- [ ] Foot terminals recolored live via OSC 4/10/11 (no restart needed)
- [ ] Wofi menu style looks right (`$mod+d`)
- [ ] Mako notification style (`notify-send test`)
- [ ] Swaylock (run `swaylock` briefly)
- [ ] Wob volume bar (tap volume keys)
- [ ] GTK apps — open Thunar (`$mod+n`) and verify window chrome matches
- [ ] Papirus folder color in Thunar matches the FOLDER_COLOR you picked
- [ ] Brave/Chromium tab strip — open the browser, the seed color should show up live without restart

Then preview the picker:

```bash
switch-theme --picker
```

## Step 7: Commit

On the `new-themes` branch:

```bash
git add sway-themes/<flavor> wallpapers/<flavor>.* bin/switch-theme
# Plus gtk-themes/<NewGtkTheme>/ if you bundled one
git commit -m "Add <THEME_NAME> theme"
git push
```

The installer will pick up the new theme on the next fresh install with no edits.

---

## Gotchas (Things Learned the Hard Way)

### Chromium/Brave `BrowserThemeColor` is a "vibe," not a palette

Chromium and Brave accept **a single color** via the managed policy `color.json`. They treat it as a seed and derive the full browser UI palette algorithmically — there is **no per-element control**.

Consequences:

- Don't expect pixel-perfect match between the browser tab strip and Thunar/GTK window bg. They will diverge on any theme whose GTK bg isn't identical to your `BASE`.
- If `BASE` doesn't produce a pleasant browser palette (e.g. very dark themes like Arc Raiders can feel washed out), use the `BROWSER_COLOR` override in the flavor file. Pick a color that represents the theme's mood rather than one that tries to match a specific pixel in another app.
- Catppuccin themes don't show this mismatch because the bundled Catppuccin GTK themes share the exact palette with the flavor file. Third-party GTK themes (Dracula, Gruvbox, Monokai) use their own internal bg shades.

This is inherent to Chromium's theming model. Don't waste time trying to fix it programmatically — it's a human color-matching task, and "close enough" is the right goal.

### GTK theme bg rarely matches your `BASE` exactly

Third-party GTK themes bake their own background colors into their CSS. Even if the theme is *named* "Dracula," its window bg may be `#1e1f29` while your flavor's `BASE` is `#282a36`. Known diffs:

- Dracula: browser `#282a36` vs GTK `#1e1f29`
- Gruvbox: browser `#282828` vs GTK `#1d2021`
- Monokai Dark: browser `#2d2a2e` vs GTK `#272822`

If you care, either fork the GTK theme and edit the CSS or pick a `BASE` that matches the GTK theme's bg. For most cases, accept the small drift.

### Foot doesn't reload from config — it uses OSC escape sequences

`reload_foot_colors()` sends OSC 4/10/11 to every running foot terminal's child-shell pty. If your new theme's colors look wrong in an open terminal after switching, check that you aren't reading an old `foot-colors.ini`. A new `foot` window will always read the freshly-rendered template; already-running windows are recolored live.

### Waybar restart, not reload

The switcher kills and restarts waybar because reload doesn't reliably pick up `style.css` changes. Expect a ~300ms blink on theme switch.

### The flavor list in `switch-theme` is hardcoded

If you forget Step 4, your theme will still work if you call `switch-theme <flavor>` directly — `apply_theme` validates against the file existing, not the list. But `--picker` will not show it, and `--help`-style listings won't include it. Always add it to the list.

### `BROWSER_COLOR` is optional — only override when needed

The default `local color="#${BROWSER_COLOR:-$BASE}"` falls back to `BASE`. Only set `BROWSER_COLOR` in the flavor file if `BASE` produces a browser palette you dislike. For most themes, leave it unset.

### Very-dark themes tend to look darker in GTK than the BASE hex implies

When a theme's palette is in the `#10-#20` range (Arc Raiders), small hex differences are visually magnified. If something looks wrong, check the actual rendered pixel before blaming the template — you're probably seeing a ±5 RGB drift that would be invisible on a lighter theme.

---

## File Reference

| Path                                    | Purpose                                          |
|-----------------------------------------|--------------------------------------------------|
| `sway-themes/<flavor>`                  | Color + metadata definitions for the theme      |
| `sway-themes/templates/*`               | Per-app config templates with `@@VAR@@` slots   |
| `wallpapers/<flavor>.*`                 | Default wallpaper, set on theme switch          |
| `gtk-themes/<GtkThemeName>/`            | Bundled GTK theme, installed to `~/.themes/`    |
| `bin/switch-theme`                      | The renderer/applier (edit `available_flavors`) |
| `~/.config/sway-themes/current`         | Runtime state: name of active flavor            |
| `~/.config/sway-themes/wallpaper-override` | User wallpaper override (beats theme default)|

## Template Variable Reference

All variables from the flavor file are available in every template as `@@VARNAME@@`. Variables currently referenced by at least one template:

`BASE`, `BLUE`, `GREEN`, `LAVENDER`, `MANTLE`, `MAUVE`, `PEACH`, `RED`, `ROSEWATER`, `SAPPHIRE`, `SKY`, `SUBTEXT0`, `SUBTEXT1`, `SURFACE0`, `SURFACE1`, `SURFACE2`, `TEAL`, `TEXT`, `THEME_NAME`, `YELLOW`

Unreferenced variables (`FLAMINGO`, `PINK`, `MAROON`, `OVERLAY0-2`, `CRUST`) are still expected in the flavor file — future template changes may pull from them, and keeping the full palette makes it easy to extend any individual template.
