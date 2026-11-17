# Infinity-Context (`icx`) Branding & Platform Assets (`assets/`)

This directory contains the branding banners, GitHub Social Preview cards, multi-resolution icons, web favicons, and desktop integration files for **Infinity-Context (`icx`)**.

## Directory Structure

```text
assets/
├── README.md
├── branding/
│   ├── icx-banner-1500x500.png          # Hero banner (3:1, 1500x500)
│   ├── icx-banner-1200x400.png          # Medium banner (3:1, 1200x400)
│   ├── icx-banner-900x300.png           # Compact banner (3:1, 900x300)
│   ├── icx-social-preview-1280x640.png  # GitHub Social Preview Card (2:1, 1280x640)
│   └── icx-social-preview-1280x640.svg  # Vector Social Preview Card (2:1, 1280x640)
├── icons/
│   ├── svg/
│   │   └── icx-icon.svg                 # Master vector icon
│   ├── png/
│   │   └── icon-{16..1024}.png          # Solid square PNG icons (16px to 1024px)
│   ├── transparent/
│   │   └── icx-icon-transparent-{16..512}.png # Rounded-rect transparent badge icons
│   └── ico/
│       ├── icx.ico                      # Multi-resolution Windows/Web .ico (16..256px)
│       └── icx-small.ico                # Compact .ico (16..48px)
├── linux/
│   ├── icx.desktop                      # Freedesktop.org .desktop launcher
│   └── icx.png                          # 256x256 Linux hicolor icon
└── web/
    ├── android-chrome-192x192.png
    ├── android-chrome-512x512.png
    ├── apple-touch-icon.png
    ├── favicon-16x16.png
    ├── favicon-32x32.png
    ├── favicon-96x96.png
    ├── favicon.ico
    └── site.webmanifest
```

## Regeneration

Run `python3 scripts/generate_assets_and_graphs.py` and `python3 scripts/render_diagrams.py` from the repository root to regenerate all assets and diagrams deterministically.
