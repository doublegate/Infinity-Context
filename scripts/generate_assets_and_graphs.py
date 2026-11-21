#!/usr/bin/env python3
"""Generate the complete nested images/, assets/, and docs/diagrams/ hierarchy for Infinity-Context (icx).

Produces:
- images/:
  - InfinityContext_Full-Logo.png
  - InfinityContext_Logo-Only.png
  - InfinityContext_Logo-Small.png
  - infinity-context.ico
  - infinity-context-architecture.dot / .png / .svg (VeridianOS Stratum-style relationship & architecture graph)
  - infinity-context-crate-graph.dot / .png / .svg (8-crate + cross-agent relationship graph)
- assets/:
  - README.md
  - branding/icx-banner-{1500x500,1200x400,900x300}.png
  - branding/icx-social-preview-1280x640.png & .svg
  - icons/svg/icx-icon.svg
  - icons/png/icon-{16,24,32,48,64,128,256,512,1024}.png
  - icons/transparent/icx-icon-transparent-{16,32,64,128,256,512}.png
  - icons/ico/icx.ico & icx-small.ico
  - web/{favicon.ico,favicon-16x16.png,favicon-32x32.png,favicon-96x96.png,apple-touch-icon.png,android-chrome-192x192.png,android-chrome-512x512.png,site.webmanifest}
  - linux/{icx.desktop,icx.png}
- docs/diagrams/:
  - README.md
  - five-plane-architecture.mmd
  - crate-and-agent-relationships.mmd
  - synchronous-hook-and-shm-flow.mmd
  - cache-epoch-lifecycle.mmd
"""

from __future__ import annotations

import json
import os
import subprocess
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter, ImageFont

ROOT = Path(__file__).resolve().parent.parent
# Directory holding the source banner/logo renders; set ICX_ASSET_SOURCE_DIR to regenerate.
ARTIFACT_DIR = Path(os.environ.get("ICX_ASSET_SOURCE_DIR", str(ROOT / "assets" / "source")))
SOURCE_BANNER = ARTIFACT_DIR / "infinity_context_banner_v2_1794884203552.jpg"
SOURCE_LOGO = ARTIFACT_DIR / "infinity_context_logo_1794884172799.jpg"


def ensure_dirs() -> None:
    dirs = [
        ROOT / "images",
        ROOT / "assets" / "branding",
        ROOT / "assets" / "icons" / "svg",
        ROOT / "assets" / "icons" / "png",
        ROOT / "assets" / "icons" / "transparent",
        ROOT / "assets" / "icons" / "ico",
        ROOT / "assets" / "web",
        ROOT / "assets" / "linux",
        ROOT / "docs" / "images",
        ROOT / "docs" / "diagrams",
    ]
    for d in dirs:
        d.mkdir(parents=True, exist_ok=True)


def make_rounded_transparent_icon(img: Image.Image, size: int) -> Image.Image:
    ss = 4
    hi = size * ss
    resized = img.convert("RGBA").resize((hi, hi), Image.Resampling.LANCZOS)
    mask = Image.new("L", (hi, hi), 0)
    draw = ImageDraw.Draw(mask)
    radius = max(8, int(hi * 0.22))
    draw.rounded_rectangle([(0, 0), (hi - 1, hi - 1)], radius=radius, fill=255)
    out = Image.new("RGBA", (hi, hi), (0, 0, 0, 0))
    out.paste(resized, (0, 0), mask=mask)
    if size >= 32:
        border_draw = ImageDraw.Draw(out)
        border_w = max(4, hi // 72)
        inset = border_w // 2
        border_draw.rounded_rectangle(
            [(inset, inset), (hi - 1 - inset, hi - 1 - inset)],
            radius=radius - inset,
            outline=(56, 189, 248, 220),
            width=border_w,
        )
    return out.resize((size, size), Image.Resampling.LANCZOS)


def crop_banner_aspect(img: Image.Image, target_w: int, target_h: int) -> Image.Image:
    src_w, src_h = img.size
    target_ratio = target_w / target_h
    src_ratio = src_w / src_h
    if src_ratio > target_ratio:
        new_w = int(src_h * target_ratio)
        left = (src_w - new_w) // 2
        cropped = img.crop((left, 0, left + new_w, src_h))
    else:
        new_h = int(src_w / target_ratio)
        top = (src_h - new_h) // 2
        cropped = img.crop((0, top, src_w, top + new_h))
    return cropped.resize((target_w, target_h), Image.Resampling.LANCZOS)


def load_font(size: int, bold: bool = False) -> ImageFont.FreeTypeFont | ImageFont.ImageFont:
    candidates = (
        [
            "/usr/share/fonts/TTF/JetBrainsMono-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono-Bold.ttf",
            "/usr/share/fonts/noto/NotoSansMono-Bold.ttf",
            "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        ]
        if bold
        else [
            "/usr/share/fonts/TTF/JetBrainsMono-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSansMono.ttf",
            "/usr/share/fonts/noto/NotoSansMono-Regular.ttf",
            "/usr/share/fonts/TTF/DejaVuSans.ttf",
        ]
    )
    for path in candidates:
        if Path(path).exists():
            return ImageFont.truetype(path, size=size)
    return ImageFont.load_default()


def build_wide_banner_3x1(banner_img: Image.Image, target_w: int = 1500, target_h: int = 500) -> Image.Image:
    """Construct a true 3:1 wide banner without clipping the torus or typography.

    Places the complete uncropped torus + 4-agent ring (CLAUDE CODE, CODEX, ANTIGRAVITY, HERMES)
    on the left and the glowing title, subtitle, AHTL wire frame, and current contract labels on the right.
    """
    base_w, base_h = 1500, 500
    bg_color = (13, 19, 31, 255)
    canvas = Image.new("RGBA", (base_w, base_h), bg_color)
    draw = ImageDraw.Draw(canvas)

    # Subtle cybernetic grid lines on the background
    for gx in range(0, base_w, 60):
        draw.line([(gx, 0), (gx, base_h)], fill=(30, 41, 59, 45), width=1)
    for gy in range(0, base_h, 60):
        draw.line([(0, gy), (base_w, gy)], fill=(30, 41, 59, 45), width=1)

    # Extract the complete torus + 4-agent ring from the upper region of source banner (1376x768)
    # y=8..548 contains the entire torus and all 4 agent nodes without the bottom title text
    torus_crop = banner_img.crop((170, 8, 1206, 548)).convert("RGBA")  # 1036 x 540
    torus_w, torus_h = 680, 354
    torus_resized = torus_crop.resize((torus_w, torus_h), Image.Resampling.LANCZOS)

    # Feather edges of torus_resized so it blends seamlessly into (13, 19, 31)
    mask = Image.new("L", (torus_w, torus_h), 0)
    mask_draw = ImageDraw.Draw(mask)
    mask_draw.rounded_rectangle([(18, 18), (torus_w - 19, torus_h - 19)], radius=48, fill=255)
    mask = mask.filter(ImageFilter.GaussianBlur(radius=14))
    canvas.paste(torus_resized, (24, (base_h - torus_h) // 2), mask)

    # Right-hand side typography & telemetry panel (x = 716 .. 1468)
    f_eyebrow = load_font(15, bold=True)
    f_title = load_font(48, bold=True)
    f_sub = load_font(18, bold=False)
    f_code = load_font(15, bold=False)
    f_pill = load_font(15, bold=True)

    rx = 716
    draw.text(
        (rx, 68),
        "CROSS-AGENT VIRTUAL CONTEXT MEMORY MANAGEMENT UNIT (MMU)",
        fill=(56, 189, 248, 255),
        font=f_eyebrow,
    )
    draw.text(
        (rx, 102),
        "INFINITY-CONTEXT",
        fill=(248, 250, 252, 255),
        font=f_title,
    )
    draw.text(
        (rx + 495, 102),
        "(ICX)",
        fill=(34, 211, 238, 255),
        font=f_title,
    )
    draw.text(
        (rx, 172),
        "Alien Holographic Token Lattice (AHTL)  |  BLAKE3 Merkle-DAG",
        fill=(203, 213, 225, 255),
        font=f_sub,
    )
    draw.text(
        (rx, 202),
        "10,240-Bit HDC + Fractal IFS Index  |  /dev/shm Seqlock Bus",
        fill=(148, 163, 184, 255),
        font=f_sub,
    )

    # AHTL Packet showcase box
    draw.rounded_rectangle(
        [(rx, 248), (1464, 332)],
        radius=10,
        fill=(15, 23, 42, 235),
        outline=(56, 189, 248, 140),
        width=2,
    )
    draw.text(
        (rx + 18, 262),
        "⟦Δ:src/auth.rs#L1-8⊢⊕fn:verify_token⋮¬unwrap()",
        fill=(192, 132, 252, 255),
        font=f_code,
    )
    draw.text(
        (rx + 18, 294),
        "↦#icx:7bf8239fb7c504f1⋮Ψ:⠒⠴⡖⡸⢚⢼⣞⣰⟧",
        fill=(52, 211, 153, 255),
        font=f_code,
    )

    # Contract labels (no performance guarantees)
    pills = [
        ("Lossy AHTL sketches", (168, 85, 247)),
        ("Post-redaction CAS", (16, 185, 129)),
        ("Bounded managed text", (245, 158, 11)),
    ]
    px = rx
    py = 362
    pw = 240
    ph = 52
    for label, (r, g, b) in pills:
        draw.rounded_rectangle(
            [(px, py), (px + pw, py + ph)],
            radius=10,
            fill=(15, 23, 42, 240),
            outline=(r, g, b, 220),
            width=2,
        )
        draw.text((px + 14, py + 16), label, fill=(241, 245, 249, 255), font=f_pill)
        px += pw + 14

    # Subtle outer frame
    draw.rounded_rectangle(
        [(10, 10), (base_w - 11, base_h - 11)],
        radius=16,
        outline=(56, 189, 248, 110),
        width=2,
    )

    if (target_w, target_h) != (base_w, base_h):
        return canvas.resize((target_w, target_h), Image.Resampling.LANCZOS)
    return canvas


def build_social_preview_png(logo_img: Image.Image, banner_img: Image.Image) -> None:
    """Create the 1280x640 GitHub Social Preview card (PNG + SVG)."""
    w, h = 1280, 640
    # Use ONLY the upper torus artwork region (y=0..540) for ambient background so bottom text never ghosts!
    torus_only = banner_img.crop((0, 0, banner_img.width, 540)).convert("RGBA")
    bg = crop_banner_aspect(torus_only, w, h)
    bg_blurred = bg.filter(ImageFilter.GaussianBlur(radius=10))
    overlay = Image.new("RGBA", (w, h), (9, 13, 22, 222))
    canvas = Image.alpha_composite(bg_blurred, overlay)
    draw = ImageDraw.Draw(canvas)

    # Outer glowing frame
    draw.rounded_rectangle(
        [(24, 24), (w - 25, h - 25)],
        radius=20,
        outline=(56, 189, 248, 180),
        width=2,
    )
    draw.rounded_rectangle(
        [(32, 32), (w - 33, h - 33)],
        radius=16,
        outline=(168, 85, 247, 90),
        width=1,
    )

    # Paste rounded logo on the left
    icon_240 = make_rounded_transparent_icon(logo_img, 240)
    canvas.paste(icon_240, (72, 130), icon_240)

    # Typography on the right
    f_tag = load_font(17, bold=True)
    f_title = load_font(52, bold=True)
    f_sub = load_font(20, bold=False)
    f_code = load_font(16, bold=False)
    f_badge = load_font(16, bold=True)

    draw.text(
        (348, 118),
        "VIRTUAL CONTEXT MEMORY MANAGEMENT UNIT (MMU) FOR AI AGENTS",
        fill=(56, 189, 248, 255),
        font=f_tag,
    )
    draw.text(
        (348, 152),
        "Infinity-Context (icx)",
        fill=(248, 250, 252, 255),
        font=f_title,
    )
    draw.text(
        (348, 226),
        "Alien Holographic Token Lattice (AHTL)  |  BLAKE3 Merkle-DAG",
        fill=(203, 213, 225, 255),
        font=f_sub,
    )
    draw.text(
        (348, 260),
        "10,240-Bit HDC + Fractal IFS Index  |  /dev/shm Seqlock Bus",
        fill=(148, 163, 184, 255),
        font=f_sub,
    )

    # AHTL sample packet strip
    draw.rounded_rectangle(
        [(348, 312), (1208, 392)],
        radius=10,
        fill=(15, 23, 42, 235),
        outline=(51, 65, 85, 255),
        width=2,
    )
    draw.text(
        (368, 326),
        "⟦Δ:src/auth.rs#L1-8⊢⊕fn:verify_token⋮¬unwrap()",
        fill=(192, 132, 252, 255),
        font=f_code,
    )
    draw.text(
        (368, 356),
        "↦#icx:7bf8239fb7c504f1⋮Ψ:⠒⠴⡖⡸⢚⢼⣞⣰⟧",
        fill=(52, 211, 153, 255),
        font=f_code,
    )

    # Contract labels along the bottom
    badges = [
        ("Lossy AHTL sketches", (168, 85, 247)),
        ("Post-redaction CAS", (16, 185, 129)),
        ("Bounded managed text", (245, 158, 11)),
        ("Lexical-first recall", (56, 189, 248)),
    ]
    bx = 72
    by = 448
    bw = 272
    bh = 58
    gap = 16
    for label, (r, g, b) in badges:
        draw.rounded_rectangle(
            [(bx, by), (bx + bw, by + bh)],
            radius=12,
            fill=(15, 23, 42, 240),
            outline=(r, g, b, 220),
            width=2,
        )
        draw.text((bx + 18, by + 19), label, fill=(241, 245, 249, 255), font=f_badge)
        bx += bw + gap

    # Footer agent bar (split cleanly left & right with no overlap)
    f_footer = load_font(15, bold=False)
    draw.text(
        (72, 552),
        "Claude Code  |  Codex CLI  |  Antigravity (agy)  |  Hermes Agent  |  MCP",
        fill=(148, 163, 184, 255),
        font=f_footer,
    )
    draw.text(
        (945, 552),
        "github.com/doublegate",
        fill=(56, 189, 248, 255),
        font=f_footer,
    )

    out_png = ROOT / "assets" / "branding" / "icx-social-preview-1280x640.png"
    canvas.convert("RGB").save(out_png, format="PNG", optimize=True)

    generate_social_preview_svg()


def generate_social_preview_svg() -> None:
    """Refresh the existing vector card labels without private raster artwork."""
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 640" width="1280" height="640">
  <title>Infinity-Context v1.2.0 candidate: symbolic codec, verified CAS and bounded managed text</title>
  <metadata>generator=scripts/generate_assets_and_graphs.py; runtime=v1.2.0-candidate; source-free SVG; contracts=docs/adr/0006-v1-2-0-context-and-compatibility.md</metadata>
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#070a12"/>
      <stop offset="50%" stop-color="#0f172a"/>
      <stop offset="100%" stop-color="#090d16"/>
    </linearGradient>
  </defs>
  <rect width="1280" height="640" fill="url(#bg)"/>
  <rect x="24" y="24" width="1232" height="592" rx="20" fill="none" stroke="#38bdf8" stroke-width="2"/>
  <rect x="32" y="32" width="1216" height="576" rx="16" fill="none" stroke="#a855f7" stroke-width="1" stroke-opacity="0.5"/>
  <text x="72" y="128" fill="#38bdf8" font-family="JetBrains Mono, monospace" font-size="18" font-weight="700">VIRTUAL CONTEXT MEMORY MANAGEMENT UNIT (MMU) FOR AI AGENTS</text>
  <text x="72" y="196" fill="#f8fafc" font-family="Inter, system-ui, sans-serif" font-size="56" font-weight="800">Infinity-Context (icx)</text>
  <text x="72" y="246" fill="#cbd5e1" font-family="Inter, system-ui, sans-serif" font-size="22">Alien Holographic Token Lattice (AHTL)  |  BLAKE3 Merkle-DAG  |  10,240-Bit HDC + Fractal IFS</text>
  <rect x="72" y="286" width="1136" height="96" rx="12" fill="#0f172a" stroke="#334155" stroke-width="2"/>
  <text x="96" y="326" fill="#c084fc" font-family="JetBrains Mono, monospace" font-size="19" font-weight="600">⟦Δ:src/auth.rs#L1-8⊢⊕fn:verify_token⋮¬unwrap()</text>
  <text x="96" y="360" fill="#34d399" font-family="JetBrains Mono, monospace" font-size="19" font-weight="600">↦#icx:7bf8239fb7c504f1⋮Ψ:⠒⠴⡖⡸⢚⢼⣞⣰⟧</text>
  <rect x="72" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#a855f7" stroke-width="2"/>
  <text x="206" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">Lossy AHTL sketches</text>
  <rect x="360" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#10b981" stroke-width="2"/>
  <text x="494" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">Post-redaction CAS</text>
  <rect x="648" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#f59e0b" stroke-width="2"/>
  <text x="782" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">Bounded managed text</text>
  <rect x="936" y="424" width="272" height="64" rx="12" fill="#0f172a" stroke="#38bdf8" stroke-width="2"/>
  <text x="1072" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">Lexical-first recall</text>
  <text x="72" y="556" fill="#94a3b8" font-family="JetBrains Mono, monospace" font-size="17">Claude Code  *  OpenAI Codex CLI  *  Google Antigravity (agy)  *  Hermes Agent  *  MCP / ACP</text>
  <text x="72" y="590" fill="#94a3b8" font-family="DejaVu Sans, sans-serif" font-size="13">v1.2.0 candidate | Managed text bound only; caller budgets external content | Contracts: docs/adr/0006</text>
</svg>
"""
    (ROOT / "assets" / "branding" / "icx-social-preview-1280x640.svg").write_text(
        svg, encoding="utf-8"
    )


def build_vector_icon_svg() -> None:
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">
  <defs>
    <linearGradient id="bg" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#070a12"/>
      <stop offset="50%" stop-color="#0f172a"/>
      <stop offset="100%" stop-color="#1e1b4b"/>
    </linearGradient>
    <linearGradient id="cyan_violet" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="#38bdf8"/>
      <stop offset="50%" stop-color="#a855f7"/>
      <stop offset="100%" stop-color="#10b981"/>
    </linearGradient>
  </defs>
  <rect x="16" y="16" width="480" height="480" rx="108" fill="url(#bg)" stroke="url(#cyan_violet)" stroke-width="10"/>
  <!-- Outer Octagonal MMU Die Frame -->
  <polygon points="160,64 352,64 448,160 448,352 352,448 160,448 64,352 64,160" fill="none" stroke="#38bdf8" stroke-opacity="0.35" stroke-width="4"/>
  <!-- Infinity Mobius Ribbon -->
  <path d="M 168 256 C 112 184, 112 328, 168 256 C 224 184, 288 328, 344 256 C 400 184, 400 328, 344 256 C 288 184, 224 328, 168 256 Z"
        fill="none" stroke="url(#cyan_violet)" stroke-width="26" stroke-linecap="round" stroke-linejoin="round"/>
  <!-- Merkle-DAG Nodes -->
  <circle cx="140" cy="256" r="12" fill="#38bdf8"/>
  <circle cx="256" cy="256" r="16" fill="#f59e0b"/>
  <circle cx="372" cy="256" r="12" fill="#10b981"/>
  <!-- Monogram -->
  <text x="256" y="416" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="44" font-weight="800" letter-spacing="4">ICX</text>
</svg>
"""
    (ROOT / "assets" / "icons" / "svg" / "icx-icon.svg").write_text(svg, encoding="utf-8")


def generate_images_and_icons() -> None:
    logo = Image.open(SOURCE_LOGO).convert("RGBA")
    banner = Image.open(SOURCE_BANNER).convert("RGBA")

    # 1. Populate images/ (VeridianOS style) — preserve full uncropped 16:9 composition
    banner.convert("RGB").save(
        ROOT / "images" / "InfinityContext_Full-Logo.png", format="PNG", optimize=True
    )
    logo.resize((1024, 1024), Image.Resampling.LANCZOS).convert("RGB").save(
        ROOT / "images" / "InfinityContext_Logo-Only.png", format="PNG", optimize=True
    )
    make_rounded_transparent_icon(logo, 256).save(
        ROOT / "images" / "InfinityContext_Logo-Small.png", format="PNG", optimize=True
    )

    # 2. Populate assets/branding/ (RustyNES style) — true 3:1 wide composite banners + full 16:9 banner
    banner.convert("RGB").save(
        ROOT / "assets" / "branding" / "icx-banner-full.png", format="PNG", optimize=True
    )
    for w, h in [(1500, 500), (1200, 400), (900, 300)]:
        b = build_wide_banner_3x1(banner, w, h)
        b.convert("RGB").save(
            ROOT / "assets" / "branding" / f"icx-banner-{w}x{h}.png",
            format="PNG",
            optimize=True,
        )

    build_social_preview_png(logo, banner)
    build_vector_icon_svg()

    # 3. Populate assets/icons/png/ and assets/icons/transparent/
    for sz in [16, 24, 32, 48, 64, 128, 256, 512, 1024]:
        solid = logo.resize((sz, sz), Image.Resampling.LANCZOS).convert("RGB")
        solid.save(ROOT / "assets" / "icons" / "png" / f"icon-{sz}.png", format="PNG", optimize=True)

    for sz in [16, 32, 64, 128, 256, 512]:
        trans = make_rounded_transparent_icon(logo, sz)
        trans.save(
            ROOT / "assets" / "icons" / "transparent" / f"icx-icon-transparent-{sz}.png",
            format="PNG",
            optimize=True,
        )

    # 4. Multi-resolution .ico files
    ico_base = make_rounded_transparent_icon(logo, 256)
    for ico_dest in [
        ROOT / "images" / "infinity-context.ico",
        ROOT / "assets" / "icons" / "ico" / "icx.ico",
        ROOT / "assets" / "web" / "favicon.ico",
    ]:
        ico_base.save(
            ico_dest,
            format="ICO",
            sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)],
        )
    make_rounded_transparent_icon(logo, 64).save(
        ROOT / "assets" / "icons" / "ico" / "icx-small.ico",
        format="ICO",
        sizes=[(16, 16), (32, 32), (48, 48)],
    )

    # 5. Web assets
    web_sizes = {
        "favicon-16x16.png": 16,
        "favicon-32x32.png": 32,
        "favicon-96x96.png": 96,
        "apple-touch-icon.png": 180,
        "android-chrome-192x192.png": 192,
        "android-chrome-512x512.png": 512,
    }
    for fname, sz in web_sizes.items():
        make_rounded_transparent_icon(logo, sz).save(
            ROOT / "assets" / "web" / fname, format="PNG", optimize=True
        )

    manifest = {
        "name": "Infinity-Context (icx)",
        "short_name": "icx",
        "description": "Cross-Agent Virtual Context MMU & Alien Holographic Token Lattice",
        "icons": [
            {"src": "/android-chrome-192x192.png", "sizes": "192x192", "type": "image/png"},
            {"src": "/android-chrome-512x512.png", "sizes": "512x512", "type": "image/png"},
        ],
        "theme_color": "#0d1117",
        "background_color": "#0d1117",
        "display": "standalone",
    }
    (ROOT / "assets" / "web" / "site.webmanifest").write_text(
        json.dumps(manifest, indent=2) + "\n", encoding="utf-8"
    )

    # 6. Linux desktop assets
    make_rounded_transparent_icon(logo, 256).save(
        ROOT / "assets" / "linux" / "icx.png", format="PNG", optimize=True
    )
    desktop_entry = """[Desktop Entry]
Type=Application
Name=Infinity-Context (icx)
GenericName=Virtual Context MMU for AI Agents
Comment=Alien Holographic Token Lattice, BLAKE3 Merkle-DAG, and /dev/shm Seqlock Telepathy Bus
Exec=icx status
Icon=icx
Terminal=true
Categories=Development;Utility;ArtificialIntelligence;
Keywords=llm;context;claude;codex;antigravity;hermes;mcp;hdc;
"""
    (ROOT / "assets" / "linux" / "icx.desktop").write_text(desktop_entry, encoding="utf-8")


def generate_graphviz_relationship_graphs() -> None:
    """Render current runtime summaries without touching branding artwork."""
    import tomllib

    header = '\n'.join([
        'digraph InfinityContext {',
        'graph [bgcolor="#0d1117", rankdir=TB, pad="0.4", nodesep="0.4", ranksep="0.6", fontcolor="#f0f6fc", fontname="DejaVu Sans", labelloc=t, label="Infinity-Context v1.2.0 candidate\\nCurrent runtime summary; dated measurements and limits: docs/numbers.md"];',
        'node [shape=box, style="rounded,filled", color="#58a6ff", fillcolor="#161b22", fontcolor="#f0f6fc", fontname="DejaVu Sans", margin="0.2,0.15"];',
        'edge [color="#8b949e", fontcolor="#8b949e", fontname="DejaVu Sans"];',
    ])
    architecture = [
        'agents [label="Claude replacement | Codex capture-only\\nagy context hooks | Hermes ContextEngine shim | MCP"];',
        'mmu [label="Validated adapter / MMU\\nCanonical icx tool; strict envelope and typed arguments"];',
        'codec [label="Plane 1: AHTL symbolic codec\\nLossy skeleton + Braille projection; exact bytes via CAS"];',
        'cas [label="Plane 2: FastCDC + BLAKE3 CAS / Merkle-DAG\\nRedact before hashing; exact post-redaction recovery"];',
        'index [label="Plane 3: Exact-token postings + HDC / fractal / PPR\\nRank lexical coverage, combined score, artifact ID\\nICXHDC04 writes; 03 reads + MMU postings rebuild"];',
        'epoch [label="Plane 4: Epoch controller + HTTP/1 JSON proxy\\nImmutable cold history; budgeted selection may change prefix"];',
        'bus [label="Plane 5: Cross-agent bus\\nIn-process atomics; file-backed positioned I/O + writer lock\\nSequence validation and explicit loss signalling"];',
        'window [label="Bounded managed window / whole-frame structured tail\\nDefault: 32768 context - 4096 completion - 4096 external\\n24576 UTF-8 bytes; unsent bus events stay queued\\nCaller budgets external content; no whole-provider-request guarantee"];',
        'store [label="Daemon SQLite WAL synchronous=FULL\\nCommit before successful ACK; errors propagate; no write-behind"];',
        'agents -> mmu; mmu -> codec; mmu -> cas; cas -> index;',
        'mmu -> epoch; mmu -> bus; index -> window; epoch -> window; bus -> window;',
        'mmu -> store; window -> agents;',
    ]
    manifests = {p.parent.name: p for p in (ROOT / "crates").glob("*/Cargo.toml")}
    crates = []
    for name in sorted(manifests):
        crates.append(f'"{name}" [label="{name}"];')
    for name, manifest in sorted(manifests.items()):
        for dep in sorted(tomllib.loads(manifest.read_text()).get("dependencies", {})):
            if dep in manifests:
                crates.append(f'"{name}" -> "{dep}";')
    crates += ['agents [label="Claude | Codex capture-only | agy | Hermes shim | MCP"];', 'agents -> "icx-cli";']
    for filename, body in [("infinity-context-architecture", architecture), ("infinity-context-crate-graph", crates)]:
        dot_file = ROOT / "images" / f"{filename}.dot"
        dot_file.write_text(header + "\n" + "\n".join(body) + "\n}\n", encoding="utf-8")
        for fmt in ("png", "svg"):
            subprocess.run(["dot", f"-T{fmt}", "-Gdpi=180", str(dot_file), "-o", str(dot_file.with_suffix(f".{fmt}"))], check=True)
            if fmt == "svg":
                import hashlib
                svg_file = dot_file.with_suffix(".svg")
                svg = svg_file.read_text(encoding="utf-8")
                start = svg.index(">", svg.index("<svg")) + 1
                digest = hashlib.sha256(dot_file.read_bytes()).hexdigest()
                metadata = f"\n<metadata>runtime=v1.2.0-candidate; source=images/{dot_file.name}; sha256={digest}; generator=scripts/generate_assets_and_graphs.py</metadata>"
                svg_file.write_text(svg[:start] + metadata + svg[start:], encoding="utf-8")


def generate_mermaid_files_and_readmes(include_asset_readme: bool = True) -> None:
    """Write reviewed runtime Mermaid templates; branding README is optional."""
    d_dir = ROOT / "docs" / "diagrams"
    (d_dir / 'five-plane-architecture.mmd').write_text('%% Infinity-Context v1.2.0 candidate; measurements: docs/numbers.md\nflowchart TB\n  subgraph AGENTS["Agent surfaces"]\n    CC["Claude Code: native output replacement"]\n    CX["Codex: PostToolUse capture only; replacement capability false"]\n    AG["Antigravity: capture and supported context hooks"]\n    HM["Hermes: user-installed ContextEngine shim"]\n  end\n  CC & CX & AG & HM --> MMU["icx-adapters: validate input and redact before persistence"]\n  MMU --> CODEC["Plane 1 / icx-codec: symbolic skeleton + Braille projection; lossy model view"]\n  MMU --> CAS["Plane 2 / icx-core: FastCDC + BLAKE3 CAS + Merkle-DAG; exact post-redaction bytes"]\n  CAS --> INDEX["Plane 3 / icx-hdc + icx-store: exact-token postings + HDC/fractal candidates + temporal PPR"]\n  INDEX --> RANK["Rank: lexical coverage, combined score, artifact ID; common postings may span corpus"]\n  INDEX --> SNAP["ICXHDC04 writes; 03 reads; MMU rebuilds missing postings from CAS event log"]\n  MMU --> EPOCH["Plane 4 / epoch controller + HTTP/1 JSON proxy: immutable cold history, budgeted selection"]\n  RANK --> WINDOW["Bounded managed window: head + selected sealed frames + recent frames + structured tail"]\n  EPOCH --> WINDOW\n  MMU --> BUS["Plane 5 / icx-bus: in-process atomics; file-backed positioned I/O and writer lock"]\n  BUS --> WINDOW\n  WINDOW --> TAIL["Whole useful tail frames; compact CAS handles; unsent bus events stay queued"]\n  MMU --> STORE["Daemon: SQLite WAL synchronous=FULL; commit before successful ACK; no write-behind"]\n  TAIL --> MCP["Canonical tool icx: validated JSON-RPC envelope and typed operation arguments"]\n  MCP --> AGENTS\n', encoding="utf-8")
    (d_dir / 'synchronous-hook-and-shm-flow.mmd').write_text('%% Infinity-Context v1.2.0 candidate; behavior differs by native adapter.\nsequenceDiagram\n  autonumber\n  participant Agent as Host agent\n  participant Adapter as icx-adapters / MMU\n  participant CAS as Redactor + BLAKE3 CAS / DAG\n  participant Index as Exact postings + HDC / fractal / PPR\n  participant Bus as File-backed bus\n  participant Store as SQLite store\n  Agent->>Adapter: Supported hook payload or canonical icx request\n  Adapter->>Adapter: Validate envelope, capability and arguments\n  Adapter->>CAS: Redact before hashing and persistence\n  CAS-->>Adapter: Recoverable artifact handle (exact post-redaction bytes)\n  Adapter->>Index: Index artifact; lexical coverage precedes similarity score\n  Adapter->>Bus: Publish bounded event (positioned I/O + writer lock)\n  Adapter->>Store: Persist successful daemon request state\n  Store-->>Adapter: Commit result (WAL synchronous=FULL)\n  alt Persistence fails\n    Adapter-->>Agent: Error response; daemon exits with error\n  else Persistence succeeds\n    alt Claude supported replacement\n      Adapter-->>Agent: Native replacement envelope\n    else Codex PostToolUse\n      Adapter-->>Agent: Non-replacing envelope; original output remains visible\n    else Other supported adapter operation\n      Adapter-->>Agent: Adapter-specific response\n    end\n  end\n  Agent->>Adapter: Next supported context assembly\n  Adapter->>Bus: Read pending events with expected sequence checks\n  Adapter->>Adapter: Select whole frames within conservative UTF-8 byte budget\n  Adapter-->>Agent: Managed window / structured tail; unsent events remain pending\n', encoding="utf-8")
    (d_dir / 'cache-epoch-lifecycle.mmd').write_text('%% Infinity-Context v1.2.0 candidate; no provider cache-hit guarantee.\nstateDiagram-v2\n  [*] --> Active: Initialize head and policy\n  Active --> Active: Accumulate pending frames below configured threshold\n  Active --> Seal: Threshold reached (default 4096) or explicit fold\n  Seal --> ColdHistory: Seal immutable epoch; retain recoverable CAS history\n  ColdHistory --> Active: Start next active epoch\n  Active --> Select: Assemble managed window\n  ColdHistory --> Select: Select sealed frames that fit policy\n  Select --> Tail: Reserve head and whole recent frames; fit useful whole tail frames\n  Tail --> Emit: Compact handles when needed; queue unsent bus events\n  Emit --> Active: Return bounded managed text\n  note right of Select\n    Default context 32768, completion 4096, external 4096.\n    Managed allowance 24576 UTF-8 bytes.\n    One byte per conservative token allowance.\n    Caller budgets external instructions, current message and tools.\n  end note\n  note right of ColdHistory\n    Cold bytes remain immutable.\n    Budgeted selection or policy changes may invalidate provider prefixes.\n    Provider requests and native host transcripts are not fully bounded here.\n  end note\n', encoding="utf-8")
    (d_dir / 'crate-and-agent-relationships.mmd').write_text('%% v1.2.0 candidate; arrows point from dependent to runtime dependency.\nflowchart TD\n  icx_adapters["icx-adapters"]\n  icx_bus["icx-bus"]\n  icx_cli["icx-cli"]\n  icx_codec["icx-codec"]\n  icx_core["icx-core"]\n  icx_hdc["icx-hdc"]\n  icx_proxy["icx-proxy"]\n  icx_store["icx-store"]\n  icx_adapters --> icx_bus\n  icx_adapters --> icx_codec\n  icx_adapters --> icx_core\n  icx_adapters --> icx_hdc\n  icx_adapters --> icx_store\n  icx_bus --> icx_core\n  icx_cli --> icx_adapters\n  icx_cli --> icx_bus\n  icx_cli --> icx_codec\n  icx_cli --> icx_core\n  icx_cli --> icx_hdc\n  icx_cli --> icx_proxy\n  icx_cli --> icx_store\n  icx_codec --> icx_core\n  icx_hdc --> icx_core\n  icx_proxy --> icx_adapters\n  icx_proxy --> icx_core\n  icx_store --> icx_core\n  Agents["Claude / Codex capture-only / agy / Hermes shim / MCP clients"] --> icx_cli\n  icx_adapters -.-> Contracts["Validated canonical icx; ICXHDC04; bounded managed window"]\n', encoding="utf-8")
    (d_dir / 'README.md').write_text('# Current runtime diagrams (v1.2.0 candidate)\n\nThese Mermaid sources describe current contracts:\n\n- [Five planes](five-plane-architecture.mmd): capture, storage, hybrid recall, bounded context and bus.\n- [Runtime crate dependencies](crate-and-agent-relationships.mmd): arrows point from dependent to dependency; excludes dev dependencies.\n- [Hook and daemon flow](synchronous-hook-and-shm-flow.mmd): adapter-specific responses and commit before successful daemon ACK.\n- [Epoch lifecycle](cache-epoch-lifecycle.mmd): immutable cold history with budgeted selection and whole-frame tails.\n\n[Graphviz relationship graphs](../../images/) are generated by `python3 scripts/generate_assets_and_graphs.py --graphs-only`. This mode does not require private branding source artwork and does not modify branding assets. The Graphviz labels and Mermaid templates are maintained in that script; the crate edges come from current Cargo manifests.\n\n[SVG illustrations](../images/) have a separate source, `scripts/render_diagrams.py`; run it to regenerate SVGs, PNGs and their mirrors under `images/`. SVG metadata records the generator source digest and candidate label. These illustrations are summaries; the subsystem prose and [ADR 0006](../adr/0006-v1-2-0-context-and-compatibility.md) define the detailed contracts.\n\nNo diagram promises a provider cache-hit rate, fixed retrieval latency, native output replacement for Codex, or a bound on an entire provider request. See [dated measurement evidence and limits](../numbers.md); synthetic byte reuse is not measured provider billing/cache behavior or model-quality assurance.\n', encoding="utf-8")
    if include_asset_readme:
        (ROOT / "assets" / "README.md").write_text(
            """# Infinity-Context (`icx`) Branding & Platform Assets (`assets/`)

    This directory contains the branding banners, GitHub Social Preview cards, multi-resolution icons, web favicons, and desktop integration files for **Infinity-Context (`icx`)**.

    See [branding status](branding/README.md): existing raster banners/social cards are archived, non-normative artwork; current vector labels and runtime diagrams supersede their embedded claims.

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
    """,
            encoding="utf-8",
        )



def main() -> None:
    import argparse

    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--graphs-only", action="store_true", help="Regenerate runtime diagrams without private branding inputs")
    modes.add_argument("--branding-svg-only", action="store_true", help="Refresh only the standalone vector social card; preserve archived rasters")
    args = parser.parse_args()
    if args.branding_svg_only:
        generate_social_preview_svg()
        print("Updated vector social card; archived branding rasters preserved.")
        return
    ensure_dirs()
    if not args.graphs_only:
        generate_images_and_icons()
    generate_graphviz_relationship_graphs()
    generate_mermaid_files_and_readmes(include_asset_readme=not args.graphs_only)
    print("Generated runtime diagrams." if args.graphs_only else "Generated all assets and diagrams.")


if __name__ == "__main__":
    main()
