# Icon generation prompt

## Files

| | |
|---|---|
| `icon-source.png` | The artwork exactly as generated. Never edited. |
| `icon.png` | What Tauri is fed: the same art recentred on its tile, with the dead margin reduced so the tile fills 88% of the canvas. |

Generated art tends to leave 10–15% transparent margin, which renders the icon
visibly smaller than its neighbours in a taskbar, and often sits a few pixels
off centre. To regenerate `icon.png` from new artwork, crop a square centred on
the *tile* (alpha > 200) sized `tile / 0.88`, then resize to 1024.

Then:

```sh
cargo tauri icon assets/icon.png -o src-tauri/icons
rm -rf src-tauri/icons/android src-tauri/icons/ios   # not target platforms
cp src-tauri/icons/128x128.png public/icon.png       # titlebar and About
```

## The prompt

Paste into Gemini (or any image model). Ask for a 1024×1024 PNG on a
transparent background.

---

Design a modern desktop application icon for "ZRename", a bulk file-renaming
tool for Linux and Windows. It previews thousands of renames in a before/after
table and can undo an entire batch, even after a restart. Its identity is a
**workshop bench**: mechanical, precise, everything labelled — a machinist's
tool, not a toy.

**Composition**
- A single square icon with generously rounded corners (roughly a 22% corner
  radius, in the style of modern macOS and Windows 11 app icons).
- One clear, centred subject with comfortable padding. No text, no letters, no
  numbers, no wordmark.
- Flat vector shapes with crisp geometry. Subtle depth is fine — a soft inner
  gradient, a fine 1px lighter rim — but no glossy 3D bevels, no drop shadows
  falling outside the square, no photorealism.

**Colour**
- Base: a very dark charcoal, near-black with a slight blue cast (#0E1013 to
  #20262E), as a soft vertical gradient.
- Subject: warm amber (#F59E0B, highlights toward #FBBF24). Amber is the only
  saturated colour — think the start button on a machine.
- Optionally one restrained secondary accent of muted green (#34D399) if the
  concept needs a second colour. Never red.

**Subject — pick the strongest of these, do not combine them**
1. A stylised letter Z built from three amber bars, where the lower horizontal
   bar terminates in an arrowhead pointing right, so the letter reads as a
   transformation.
2. Two short stacked horizontal bars on the left and two on the right, joined
   by a bold amber arrow pointing right — an abstract before/after rename,
   like a tiny table of names becoming other names.
3. A file or tag outline with an amber arrow curving through it, suggesting a
   name being changed and able to change back.

**Must survive being small.** It will be shown at 32×32 and 16×16. Use few,
large shapes with thick strokes and high contrast against the dark base. No
thin lines, no fine detail, no gradients that muddy when downscaled.

**Avoid**: gears, wrenches, magnifying glasses, folders with generic document
sheets, clip-art file icons, stock "AI" glows, neon, drop shadows, busy
backgrounds, any lettering.

Output: 1024×1024, square, transparent outside the rounded corners.
