# cece: a Burmilla cat for Omarchy

Cece is a cat that chases your mouse cursor across the desktop. It is a Rust
port of [crgimenes/neko](https://github.com/crgimenes/neko), rebuilt for
Hyprland and packaged as an [Omarchy](https://omarchy.org) bar-widget plugin.
Cece is a **Burmilla**: silver-white coat, darker tipping on the back and
crown, green eyes lined in black and a brick-pink nose.

## What changed from neko

| | crgimenes/neko (Go) | cece (Rust) |
|---|---|---|
| Display | Ebitengine window moved around with `SetWindowPosition` (does nothing on Wayland) | `wlr-layer-shell` overlay surface, click-through, above everything |
| Cursor | window-relative cursor | Hyprland IPC `cursorpos`, global across monitors |
| Motion | fixed step every tick; two frames flipped on a timer | eased steering with braking on arrival, legs cycle with the distance covered, a small bounce each stride, 60+ fps on the monitor's refresh clock |
| Idle | awake → scratch → wash → yawn → sleep | the same routine with some randomness, blinking, a head-bob while washing, a startled hop when woken, floating Zzz, clawing at the screen edge when the cursor is out of reach |
| Sprites | 32×32, black and white | 128×128 Burmilla with 2px lines, pixel-exact on a 2× HiDPI screen at the default size (or `--skin classic`), plus a soft shadow |

## Install

You need Rust (`cargo`) and Hyprland. On Omarchy:

```bash
./install.sh
```

This builds `cece`, installs it to `~/.local/bin/cece`, copies the bar widget
to `~/.config/omarchy/plugins/ure.cece` and adds it to the right side of the bar.
The cat comes out straight away.

The bar icon:

- **Click** to mute or unmute the cat. The icon greys out while it is muted.
- **Right click** to make it stay put, or follow again.
- **Middle click** to let it out or send it away.

Sound and whether the cat is out are remembered across shell restarts and
logins. Widget settings (speed, size, skin) live in the `ure.cece` entry of
`~/.config/omarchy/shell.json`, for example:

```json
{ "id": "ure.cece", "speed": 3, "scale": 2.5, "quiet": true }
```

`./install.sh --no-plugin` installs only the binary, and `./install.sh --uninstall`
removes both.

## Running it by hand

```bash
cece                 # Burmilla, default speed and size
cece --skin classic  # the original black-and-white neko
cece --speed 3 --scale 1.5 --quiet
pkill -USR1 -x cece  # toggle stay / follow
pkill -USR2 -x cece  # toggle sound
```

Defaults can also go in `~/.config/cece/config`:

```ini
speed = 2.5
scale = 2
quiet = true
skin = burmilla
```

## Sprites

`sprites/classic/` holds the original 32×32 neko sprites. `tools/burmilla.py`
(needs Pillow) turns them into the sheets under `assets/`:

1. It sorts every pixel into outline, inner line, fur, eye or nose. Eyes and
   nose are found as the small isolated marks inside the face.
2. It scales the pixel classes 4× with Scale2x applied twice, which keeps the
   curves and diagonals smooth. Then it thins the lines again: the outline
   keeps its outer 2px, and inner lines are reduced to a skeleton and redrawn
   at an even 2px.
3. It paints the coat at 128×128 in flat bands, with no dithering. Fur darkens
   the closer it sits to the top edge of the silhouette, which gives the back,
   crown and tail their tipping. Fur far from any edge becomes the pale chest.
   Eyes are green with black lids and a glint.
4. It adds blink frames and writes `assets/burmilla.png` and `assets/classic.png`.

```bash
python3 tools/burmilla.py --preview /tmp/preview.png
cargo build --release
```

A custom skin is any PNG laid out like those sheets: 128×128 frames, eight per
row, in the order listed in `src/sprites.rs`. Load it with `--skin path.png`.

## License

BSD 2-Clause, like neko. The sprites and sounds come from crgimenes/neko,
which in turn inherited them from the classic Neko.
