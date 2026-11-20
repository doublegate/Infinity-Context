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
    on the left and the glowing title, subtitle, AHTL wire frame, and architectural metrics on the right.
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
        "<<AHTLv1 #icx:7bf8239fb7c504f1 !0.84 @0123:0210 [Braille-256:64b]",
        fill=(192, 132, 252, 255),
        font=f_code,
    )
    draw.text(
        (rx + 18, 294),
        "  $fn:verify_ed25519_token(tok:&[u8])->Result<Claims,AuthErr> !err:401>>",
        fill=(52, 211, 153, 255),
        font=f_code,
    )

    # 3 Metric Pills
    pills = [
        ("214x synthetic Compression", (168, 85, 247)),
        ("100% Lossless BLAKE3 CAS", (16, 185, 129)),
        ("95.5% byte reuse (proxy)", (245, 158, 11)),
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
        "<<AHTLv1 #icx:7bf8239fb7c504f1 !0.84 @0123:0210 [Braille-256:64b]",
        fill=(192, 132, 252, 255),
        font=f_code,
    )
    draw.text(
        (368, 356),
        "  $fn:verify_ed25519_token(tok:&[u8])->Result<Claims,AuthErr> !err:401>>",
        fill=(52, 211, 153, 255),
        font=f_code,
    )

    # Metric pill badges along the bottom
    badges = [
        ("214x AHTL (synthetic)", (168, 85, 247)),
        ("100% Lossless BLAKE3 CAS", (16, 185, 129)),
        ("95.5% byte reuse (proxy)", (245, 158, 11)),
        ("HDC Scan ~62 us / 1K nodes", (56, 189, 248)),
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

    # Also write SVG version
    svg = """<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1280 640" width="1280" height="640">
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
  <text x="96" y="326" fill="#c084fc" font-family="JetBrains Mono, monospace" font-size="19" font-weight="600">«AHTLv1 #icx:7bf8239fb7c504f1 !0.84 @0123:0210 ⟦⣿⣷⣯⣟⡿⢿⣻⣽⟧</text>
  <text x="96" y="360" fill="#34d399" font-family="JetBrains Mono, monospace" font-size="19" font-weight="600">  §fn:verify_ed25519_token(tok:&amp;[u8])-&gt;Result&lt;Claims,AuthErr&gt; !err:401»</text>
  <rect x="72" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#a855f7" stroke-width="2"/>
  <text x="206" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">214x AHTL (synthetic)</text>
  <rect x="360" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#10b981" stroke-width="2"/>
  <text x="494" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">100% Lossless BLAKE3</text>
  <rect x="648" y="424" width="268" height="64" rx="12" fill="#0f172a" stroke="#f59e0b" stroke-width="2"/>
  <text x="782" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">95.5% byte reuse (proxy)</text>
  <rect x="936" y="424" width="272" height="64" rx="12" fill="#0f172a" stroke="#38bdf8" stroke-width="2"/>
  <text x="1072" y="463" text-anchor="middle" fill="#f8fafc" font-family="JetBrains Mono, monospace" font-size="17" font-weight="700">HDC Scan ~62 us / 1K nodes</text>
  <text x="72" y="556" fill="#94a3b8" font-family="JetBrains Mono, monospace" font-size="17">Claude Code  *  OpenAI Codex CLI  *  Google Antigravity (agy)  *  Hermes Agent  *  MCP / ACP</text>
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
    """Write and compile VeridianOS-style Graphviz (.dot -> .png & .svg) relationship graphs."""
    arch_dot = r"""digraph InfinityContextArchitecture {
    // =================================================================
    // Infinity-Context (icx) — Virtual Context MMU & 5-Plane Architecture
    // Style: Dark theme matching VeridianOS Stratum ecosystem diagram
    // Note: Every multi-line cell uses separate <TR><TD ALIGN="LEFT"> rows
    //       to avoid Graphviz's <BR/> center-alignment quirk.
    // =================================================================

    bgcolor="#0d1117"
    rankdir=TB
    splines=true
    nodesep=0.70
    ranksep=1.00
    pad="0.6,0.4"
    compound=true
    labelloc="t"
    label=<
        <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f0f6fc" POINT-SIZE="18" FACE="JetBrains Mono,Consolas,monospace"><B>Infinity-Context (icx) &#8212; 5-Plane Virtual Context MMU Architecture</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="11" FACE="JetBrains Mono,Consolas,monospace">(214x synthetic AHTL compression &#183; lossless (post-redaction) BLAKE3 CAS &#183; 10,240-bit HDC &#183; 95.5% byte reuse (proxy) &#183; ~60 ns in-process publish /dev/shm seqlock)</FONT></TD></TR>
        </TABLE>>

    node [shape=none fontname="JetBrains Mono,Consolas,Courier New,monospace" fontsize=10 margin="0"]
    edge [arrowsize=0.75 penwidth=1.6 fontname="JetBrains Mono,Consolas,monospace" fontsize=8.5]

    // =================================================================
    // LEGEND
    // =================================================================
    legend [label=<
        <TABLE BORDER="1" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#30363d" BGCOLOR="#161b22">
        <TR><TD COLSPAN="2" ALIGN="CENTER"><FONT COLOR="#f0f6fc" POINT-SIZE="10"><B>Architectural Planes</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Blue: Host AI Agents &amp; CLI Surfaces</FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#a371f7">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Purple: Plane 1 &#8212; AHTL Neural Codec</FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#39d2c0">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Teal: Plane 2 &#8212; BLAKE3 CAS &amp; Merkle-DAG</FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#2ea043">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Green: Plane 3 &#8212; 10,240-Bit HDC + Fractal + PPR</FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#d29922">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Amber: Plane 4 &#8212; Prefix-Cache-Locked Wire Proxy</FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f85149">&#9679;</FONT></TD><TD ALIGN="LEFT"><FONT COLOR="#8b949e" POINT-SIZE="9">Red: Plane 5 &#8212; /dev/shm Seqlock Telepathy Bus</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // HOST AGENTS (Blue #58a6ff)
    // =================================================================
    claude [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="11"><B>Claude Code CLI</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; PostToolUse (updatedMCPToolOutput)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; UserPromptSubmit &amp; PreCompact</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; SessionStart &amp; Stop Lifecycle</FONT></TD></TR>
        </TABLE>>]

    codex [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="11"><B>OpenAI Codex CLI</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; ~/.codex/hooks.json Interception</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; state_5.sqlite + memories_1.sqlite</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; OPENAI_BASE_URL Cache Wire Proxy</FONT></TD></TR>
        </TABLE>>]

    agy [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="11"><B>Google Antigravity (agy)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; ~/.gemini/config/hooks.json (exit code 0)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; injectSteps JSON Envelope</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Protobuf Jetski State Sync</FONT></TD></TR>
        </TABLE>>]

    hermes [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="11"><B>Hermes Agent + MCP</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; MemoryProvider Python Bridge</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; state.db + mind.db FTS5 Store</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Single-Tool icx(op,ref,q,k)</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // PLANE 1: AHTL NEURAL CODEC (Purple #a371f7)
    // =================================================================
    redactor [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#a371f7" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#a371f7" POINT-SIZE="11"><B>SecretRedactor + Sanitizer</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Strips sk-ant-*, sk-proj-*, ghp_*</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Blocks Prompt-Injection Delimiters</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Pre-Hash Boundary Enforcement</FONT></TD></TR>
        </TABLE>>]

    skeletonizer [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#a371f7" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#a371f7" POINT-SIZE="11"><B>StructuralSkeletonizer</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Extracts $fn / $ty / $mod / $use</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Preserves !err / !panic / !assert</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Key-Value &amp; Numeric Extraction</FONT></TD></TR>
        </TABLE>>]

    ahtl_codec [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6" COLOR="#a371f7" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#a371f7" POINT-SIZE="12"><B>Plane 1: AHTL Packet Codec (icx-codec)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; &lt;&lt;AHTLv1 #icx:hash !surp @fcga [Braille-256] $...&gt;&gt;</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; U+2800..U+28FF Bijection (8 bits/glyph, 64-bit sketch)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; 214&#215; packet ratio on a synthetic test log</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // PLANE 2: VIRTUAL MEMORY & MERKLE-DAG (Teal #39d2c0)
    // =================================================================
    fastcdc [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#39d2c0" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#39d2c0" POINT-SIZE="11"><B>FastCDC + BLAKE3 CAS</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Gear-Hash Content-Defined Chunking</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; 64-Bit #icx:&lt;16-hex&gt; Handle</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; SQLite store (icx.sqlite3, WAL)</FONT></TD></TR>
        </TABLE>>]

    merkle_dag [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6" COLOR="#39d2c0" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#39d2c0" POINT-SIZE="12"><B>Plane 2: MerkleContextDag (icx-core)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; L0 RawChunk &#8594; L1 EpisodeNode &#8594; L2 FoldedSubtask &#8594; L3 EpochRoot</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Context-Folding Subtask Rollback (90% Token Savings)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; 100% Lossless O(1) Page-In Dereferencing</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // PLANE 3: HDC + FRACTAL + PPR INDEX (Green #2ea043)
    // =================================================================
    hdc_engine [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6" COLOR="#2ea043" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#2ea043" POINT-SIZE="12"><B>Plane 3: 10,240-Bit HDC + Fractal IFS (icx-hdc)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Hypervector([u64; 160]) &#183; XOR Bind &#183; POPCNT Scan (~64 ns/node)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Infini-Attention Delta Bundler (sim &lt; 0.85 orthogonality)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Contractive IFS FractalCoord + TitansSurpriseGate</FONT></TD></TR>
        </TABLE>>]

    ppr_graph [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#2ea043" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#2ea043" POINT-SIZE="11"><B>Bi-Temporal PPR Graph (icx-store)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Valid Window [t_valid, t_invalid)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Tx Window [t_created, t_expired)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; HippoRAG Personalized PageRank (&#945;=0.15)</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // PLANE 4 & PLANE 5: CACHE EPOCH + SHM BUS (Amber #d29922 / Red #f85149)
    // =================================================================
    epoch_proxy [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6" COLOR="#d29922" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#d29922" POINT-SIZE="12"><B>Plane 4: Cache-Epoch Wire Proxy (icx-proxy)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Byte-Frozen Prefix Epochs (0 Mid-Session Edits)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Anthropic 4-Slot cache_control: ephemeral</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; OpenAI 1,024-Token Aligned Prefix (95.5% byte reuse, proxy)</FONT></TD></TR>
        </TABLE>>]

    shm_bus [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6" COLOR="#f85149" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f85149" POINT-SIZE="12"><B>Plane 5: /dev/shm Seqlock Telepathy Bus (icx-bus)</B></FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx-fabric-v1.shm (256-slot seqlock ring file)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; 256 &#215; 4,096-Byte Cache-Aligned Atomic Slots</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; ~38 us file-backed Cross-Agent HDC &amp; Handle Broadcast</FONT></TD></TR>
        </TABLE>>]

    // =================================================================
    // EDGES
    // =================================================================
    claude -> redactor [color="#58a6ff" style=bold]
    codex -> redactor [color="#58a6ff" style=bold]
    agy -> redactor [color="#58a6ff" style=bold]
    hermes -> redactor [color="#58a6ff" style=bold]

    redactor -> skeletonizer [color="#a371f7" style=bold]
    skeletonizer -> ahtl_codec [color="#a371f7" style=bold]
    redactor -> fastcdc [color="#39d2c0" style=bold]

    fastcdc -> merkle_dag [color="#39d2c0" style=bold]
    merkle_dag -> ahtl_codec [color="#39d2c0" style=bold]

    ahtl_codec -> hdc_engine [color="#2ea043" style=bold]
    merkle_dag -> hdc_engine [color="#2ea043" style=dashed]
    hdc_engine -> ppr_graph [color="#2ea043" style=bold]

    ahtl_codec -> epoch_proxy [color="#d29922" style=bold]
    ppr_graph -> epoch_proxy [color="#d29922" style=bold]
    merkle_dag -> epoch_proxy [color="#d29922" style=dashed]

    hdc_engine -> shm_bus [color="#f85149" style=bold]
    shm_bus -> claude [color="#f85149" style=dashed]
    shm_bus -> codex [color="#f85149" style=dashed]
    shm_bus -> agy [color="#f85149" style=dashed]
    shm_bus -> hermes [color="#f85149" style=dashed]

    { rank=same; claude; codex; agy; hermes; legend }
    { rank=same; redactor; skeletonizer; fastcdc }
    { rank=same; ahtl_codec; merkle_dag }
    { rank=same; hdc_engine; ppr_graph }
    { rank=same; epoch_proxy; shm_bus }
}
"""

    crate_dot = r"""digraph InfinityContextCrateGraph {
    // =================================================================
    // Infinity-Context (icx) — 8-Crate Workspace & Cross-Agent Dependency DAG
    // Style: Dark theme matching VeridianOS relationship diagrams
    // Note: Every bullet uses its own <TR><TD ALIGN="LEFT"> row for exact left alignment.
    // =================================================================

    bgcolor="#0d1117"
    rankdir=LR
    splines=true
    nodesep=0.55
    ranksep=1.10
    pad="0.6,0.4"
    labelloc="t"
    label=<
        <TABLE BORDER="0" CELLBORDER="0" CELLSPACING="0" CELLPADDING="6">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f0f6fc" POINT-SIZE="17" FACE="JetBrains Mono,Consolas,monospace"><B>Infinity-Context (icx) &#8212; 8-Crate Workspace &amp; Subsystem Dependency DAG</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="10" FACE="JetBrains Mono,Consolas,monospace">(Strict acyclic crate hierarchy &#183; unsafe_code = forbid in all 8 crates)</FONT></TD></TR>
        </TABLE>>

    node [shape=none fontname="JetBrains Mono,Consolas,Courier New,monospace" fontsize=10 margin="0"]
    edge [arrowsize=0.75 penwidth=1.6]

    icx_cli [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#f0f6fc" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f0f6fc" POINT-SIZE="12"><B>icx-cli</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Binary Entry Point]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx hook &lt;agent&gt; &lt;event&gt;</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx mcp-serve (JSON-RPC)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx proxy-transform</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx encode-ahtl / decode-ahtl</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; icx doctor / benchmark / status</FONT></TD></TR>
        </TABLE>>]

    icx_adapters [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="11"><B>icx-adapters</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Agent &amp; MCP Orchestration]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; InfinityContextMmu</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; ClaudeCodeAdapter</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; CodexAdapter</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; AntigravityAdapter (exit 0)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; HermesAdapter + McpPagerServer</FONT></TD></TR>
        </TABLE>>]

    icx_proxy [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#d29922" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#d29922" POINT-SIZE="11"><B>icx-proxy</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Cache-Epoch Wire Proxy]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; WireProxyTransformer</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Anthropic 4-Breakpoint Cache</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; OpenAI 1024-Tok Prefix Align</FONT></TD></TR>
        </TABLE>>]

    icx_codec [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#a371f7" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#a371f7" POINT-SIZE="11"><B>icx-codec</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[AHTL Symbolic Codec]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; AhtlPacket (encode/parse)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Braille-256 Bijection</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; StructuralSkeletonizer</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; SecretRedactor</FONT></TD></TR>
        </TABLE>>]

    icx_store [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#2ea043" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#2ea043" POINT-SIZE="11"><B>icx-store</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Bi-Temporal Graph &amp; PPR]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; BiTemporalGraph</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; TemporalEdge (Valid/Tx)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; PersonalizedPageRank</FONT></TD></TR>
        </TABLE>>]

    icx_bus [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#f85149" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#f85149" POINT-SIZE="11"><B>icx-bus</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Seqlock Ring Bus]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; SeqlockTelemetryBus</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; SeqlockSlot (4,096 B)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Safe Rust (pread/pwrite + flock)</FONT></TD></TR>
        </TABLE>>]

    icx_hdc [label=<
        <TABLE BORDER="2" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#39d2c0" BGCOLOR="#0d1117">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#39d2c0" POINT-SIZE="11"><B>icx-hdc</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[10,240-Bit VSA &amp; Fractal IFS]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; Hypervector ([u64; 160])</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; HdcBundler (Delta Rule)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; FractalCoord (16-D IFS)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; TitansSurpriseGate</FONT></TD></TR>
        </TABLE>>]

    icx_core [label=<
        <TABLE BORDER="3" CELLBORDER="0" CELLSPACING="0" CELLPADDING="5" COLOR="#58a6ff" BGCOLOR="#161b22">
        <TR><TD ALIGN="CENTER"><FONT COLOR="#58a6ff" POINT-SIZE="12"><B>icx-core</B></FONT></TD></TR>
        <TR><TD ALIGN="CENTER"><FONT COLOR="#8b949e" POINT-SIZE="8.5">[Foundational Types &amp; CAS DAG]</FONT></TD></TR>
        <HR/>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; IcxHandle (#icx:&lt;16-hex&gt;)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; FastCdcChunker + CasStore</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; MerkleContextDag (L0..L3)</FONT></TD></TR>
        <TR><TD ALIGN="LEFT"><FONT COLOR="#c9d1d9" POINT-SIZE="9">&#8226; CacheEpochController</FONT></TD></TR>
        </TABLE>>]

    // Strict Acyclic Crate Dependency Edges
    icx_cli -> icx_adapters [color="#f0f6fc" style=bold]
    icx_cli -> icx_proxy [color="#d29922" style=bold]
    icx_cli -> icx_codec [color="#a371f7"]
    icx_cli -> icx_bus [color="#f85149"]
    icx_cli -> icx_store [color="#2ea043"]
    icx_cli -> icx_hdc [color="#39d2c0"]
    icx_cli -> icx_core [color="#58a6ff"]

    icx_adapters -> icx_codec [color="#a371f7" style=bold]
    icx_adapters -> icx_store [color="#2ea043" style=bold]
    icx_adapters -> icx_bus [color="#f85149" style=bold]
    icx_adapters -> icx_hdc [color="#39d2c0" style=bold]
    icx_adapters -> icx_core [color="#58a6ff" style=bold]

    icx_proxy -> icx_core [color="#d29922" style=bold]

    icx_codec -> icx_hdc [color="#a371f7" style=bold]
    icx_codec -> icx_core [color="#a371f7" style=bold]

    icx_store -> icx_hdc [color="#2ea043" style=bold]
    icx_store -> icx_core [color="#2ea043" style=bold]

    icx_bus -> icx_hdc [color="#f85149" style=bold]
    icx_bus -> icx_core [color="#f85149" style=bold]

    icx_hdc -> icx_core [color="#39d2c0" style=bold penwidth=2.2]
}
"""

    arch_dot_path = ROOT / "images" / "infinity-context-architecture.dot"
    crate_dot_path = ROOT / "images" / "infinity-context-crate-graph.dot"
    # Add non-breaking right-padding inside every left-aligned table cell so SVG/PNG renderers have generous right margin
    arch_dot_padded = arch_dot.replace("</FONT></TD></TR>", "&#160;&#160;&#160;</FONT></TD></TR>")
    crate_dot_padded = crate_dot.replace("</FONT></TD></TR>", "&#160;&#160;&#160;</FONT></TD></TR>")
    arch_dot_path.write_text(arch_dot_padded, encoding="utf-8")
    crate_dot_path.write_text(crate_dot_padded, encoding="utf-8")

    for dot_file in [arch_dot_path, crate_dot_path]:
        png_out = dot_file.with_suffix(".png")
        svg_out = dot_file.with_suffix(".svg")
        subprocess.run(
            ["dot", "-Tpng", "-Gdpi=180", str(dot_file), "-o", str(png_out)],
            check=True,
        )
        subprocess.run(
            ["dot", "-Tsvg", str(dot_file), "-o", str(svg_out)],
            check=True,
        )


def generate_mermaid_files_and_readmes() -> None:
    """Create standalone .mmd files in docs/diagrams/ and READMEs in docs/diagrams/ and assets/."""
    d_dir = ROOT / "docs" / "diagrams"

    (d_dir / "five-plane-architecture.mmd").write_text(
        """%%{init: {
  "theme": "dark",
  "themeVariables": {
    "fontFamily": "JetBrains Mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace",
    "fontSize": "13px",
    "primaryColor": "#0f172a",
    "primaryTextColor": "#f8fafc",
    "primaryBorderColor": "#38bdf8",
    "lineColor": "#64748b",
    "secondaryColor": "#1e1b4b",
    "tertiaryColor": "#064e3b"
  },
  "flowchart": { "curve": "basis", "nodeSpacing": 45, "rankSpacing": 60 }
}}%%

flowchart TB
  subgraph AGENTS["Host AI Coding & Research Agents"]
    direction LR
    CC["Claude Code CLI<br/>PostToolUse / UserPromptSubmit<br/>PreCompact / SessionStart / Stop"]
    CX["OpenAI Codex CLI<br/>~/.codex/hooks.json<br/>state_5.sqlite + Wire Proxy"]
    AG["Google Antigravity (agy)<br/>~/.gemini/config/hooks.json (exit 0)<br/>injectSteps Envelope"]
    HM["Hermes Agent + MCP<br/>MemoryProvider Bridge<br/>state.db + mind.db"]
  end

  subgraph P1["Plane 1: Alien Holographic Token Lattice (icx-codec)"]
    direction LR
    RED["SecretRedactor<br/>Sanitizes API Keys &amp;<br/>Prompt-Injection Guards"]
    SKEL["StructuralSkeletonizer<br/>Extracts §fn / §ty / §mod<br/>Preserves !err / !panic"]
    AHTL["AhtlPacket Codec<br/>«AHTLv1 #icx:hash !surp @fcga<br/>⟦Braille-256⟧ §...» (214x synthetic)"]
  end

  subgraph P2["Plane 2: Virtual Context Memory & Merkle-DAG (icx-core)"]
    direction LR
    CDC["FastCdcChunker + CasStore<br/>Gear-Hash Rolling CDC<br/>BLAKE3 64-Bit #icx:handle"]
    DAG["MerkleContextDag (LCM)<br/>L0 Raw -> L1 Episode -> L2 Folded -> L3 Epoch<br/>100% Lossless O(1) Page-In"]
  end

  subgraph P3["Plane 3: Associative Recall & Temporal Graph (icx-hdc + icx-store)"]
    direction LR
    HDC["10,240-Bit Hypervector VSA<br/>Infini-Attention Delta Bundler<br/>POPCNT Scan (~62 us / 1K nodes)"]
    FCGA["Contractive IFS FractalCoord<br/>+ TitansSurpriseGate<br/>Momentum S_t = eta*S_{t-1} + (1-eta)*surp"]
    PPR["Bi-Temporal Graph + HippoRAG PPR<br/>Valid [t_valid, t_invalid)<br/>Tx [t_created, t_expired)"]
  end

  subgraph P45["Plane 4 & Plane 5: Cache-Epoch Wire Proxy & /dev/shm Seqlock Fabric"]
    direction LR
    EPOCH["CacheEpochController + WireProxyTransformer<br/>Byte-Frozen Prefix Epochs (95.5% byte reuse, proxy)<br/>Anthropic 4-Slot &amp; OpenAI 1024-Tok Prefix"]
    SHM["SeqlockTelemetryBus (icx-bus)<br/>icx-fabric-v1.shm (256-slot seqlock ring file)<br/>~38 us file-backed Cross-Agent Telepathy"]
  end

  CC & CX & AG & HM --> RED
  RED --> SKEL --> AHTL
  RED --> CDC --> DAG
  DAG --> AHTL
  AHTL --> HDC --> FCGA --> PPR
  AHTL & PPR & DAG --> EPOCH
  HDC <--> SHM
  SHM -.-> CC & CX & AG & HM
""",
        encoding="utf-8",
    )

    (d_dir / "crate-and-agent-relationships.mmd").write_text(
        """%%{init: {"theme": "dark", "flowchart": {"curve": "basis"}}}%%
flowchart LR
  subgraph CLI_LAYER["Binary & Wire Entry Points"]
    CLI["icx-cli<br/>(icx binary)"]
  end

  subgraph ORCH_LAYER["Adapter & Proxy Orchestration"]
    ADAPT["icx-adapters<br/>InfinityContextMmu<br/>Claude / Codex / agy / Hermes<br/>McpPagerServer"]
    PROXY["icx-proxy<br/>WireProxyTransformer<br/>Anthropic &amp; OpenAI Cache-Epoch"]
  end

  subgraph ENGINE_LAYER["Codec, Graph & IPC Engines"]
    CODEC["icx-codec<br/>AhtlPacket + Braille-256<br/>StructuralSkeletonizer<br/>SecretRedactor"]
    STORE["icx-store<br/>BiTemporalGraph<br/>PersonalizedPageRank"]
    BUS["icx-bus<br/>SeqlockTelemetryBus<br/>/dev/shm mmap Ring"]
  end

  subgraph CORE_LAYER["Mathematical & Cryptographic Foundation"]
    HDC["icx-hdc<br/>Hypervector ([u64; 160])<br/>HdcBundler + FractalCoord<br/>TitansSurpriseGate"]
    CORE["icx-core<br/>IcxHandle + FastCdcChunker<br/>CasStore + MerkleContextDag<br/>CacheEpochController"]
  end

  CLI --> ADAPT & PROXY & CODEC & STORE & BUS & HDC & CORE
  ADAPT --> CODEC & STORE & BUS & HDC & CORE
  PROXY --> CODEC & CORE
  CODEC --> HDC & CORE
  STORE --> HDC & CORE
  BUS --> HDC & CORE
  HDC --> CORE
""",
        encoding="utf-8",
    )

    (d_dir / "synchronous-hook-and-shm-flow.mmd").write_text(
        """sequenceDiagram
  autonumber
  participant Agent as Host Agent (Claude / Codex / agy / Hermes)
  participant Hook as icx-adapters (Hook / MCP)
  participant Codec as icx-codec (Redactor + AHTL)
  participant DAG as icx-core (BLAKE3 CAS + Merkle-DAG)
  participant HDC as icx-hdc + icx-store (10,240-Bit VSA + PPR)
  participant SHM as icx-bus (/dev/shm Seqlock Ring)

  Agent->>Hook: PostToolUse / MCP Tool Output (e.g., 50 KB cargo test log)
  Hook->>Codec: SecretRedactor::sanitize(raw_payload)
  Codec-->>Hook: Sanitized text + redaction count
  Hook->>DAG: CasStore::put_bytes() + MerkleContextDag::insert_raw_chunk()
  DAG-->>Hook: IcxHandle (#icx:7bf8239fb7c504f1)
  Hook->>HDC: Hypervector::from_text() + FractalCoord::project() + TitansSurpriseGate::observe()
  HDC->>HDC: BiTemporalGraph::upsert_node() + PersonalizedPageRank
  Hook->>Codec: AhtlPacket::encode() (Braille-256 + §fn/§ty + !err)
  Codec-->>Hook: Compact AHTL Packet (214x synthetic)
  Hook->>SHM: SeqlockTelemetryBus::publish(SeqlockSlot [2048 B])
  SHM-->>Agent: Cross-Agent Telepathy Broadcast (~38 us file-backed)
  Hook-->>Agent: Return JSON Envelope (updatedMCPToolOutput / AdditionalContext, exit 0)
""",
        encoding="utf-8",
    )

    (d_dir / "cache-epoch-lifecycle.mmd").write_text(
        """stateDiagram-v2
  [*] --> Epoch0_Genesis: SessionStart / Initial System + Frozen Epoch 0
  Epoch0_Genesis --> ActiveEpoch_Accumulating: Append Turn Delta & AHTL Page Table
  ActiveEpoch_Accumulating --> ActiveEpoch_Accumulating: Pending Tokens < epoch_token_threshold (16,384)
  ActiveEpoch_Accumulating --> EpochBoundary_Seal: Pending Tokens >= 16,384 OR Subtask Fold
  EpochBoundary_Seal -->MerkleL3_Committed: Seal L3 EpochRoot (#icx:epoch_hash) + Pin cache_control
  MerkleL3_Committed --> ActiveEpoch_Accumulating: Open Epoch N+1 (Prefix 0..N 100% Byte-Frozen)
  ActiveEpoch_Accumulating --> PreCompact_Intercept: Host Agent Fires PreCompact Hook
  PreCompact_Intercept --> MerkleL3_Committed: Emit Lossless AHTL Epoch Manifest (Zero Summary Loss)
""",
        encoding="utf-8",
    )

    (d_dir / "README.md").write_text(
        """# Architecture & Relationship Diagrams (`docs/diagrams/`)

This directory contains normative Mermaid (`.mmd`) diagram sources for **Infinity-Context (`icx`)**:

- [`five-plane-architecture.mmd`](five-plane-architecture.mmd) — Complete 5-plane Virtual Context MMU dataflow across Claude Code, OpenAI Codex CLI, Google Antigravity CLI (`agy`), and Hermes Agent.
- [`crate-and-agent-relationships.mmd`](crate-and-agent-relationships.mmd) — Strict acyclic 8-crate Rust workspace dependency graph (`icx-core` through `icx-cli`).
- [`synchronous-hook-and-shm-flow.mmd`](synchronous-hook-and-shm-flow.mmd) — Sequence diagram of synchronous `PostToolUse` / `UserPromptSubmit` hook interception, BLAKE3 CAS storage, AHTL encoding, and `/dev/shm` Seqlock broadcast.
- [`cache-epoch-lifecycle.mmd`](cache-epoch-lifecycle.mmd) — State machine of the Prefix-Cache-Locked Epoch Controller (95.5% prefix byte reuse, synthetic proxy).

High-resolution Graphviz (`.dot`, `.png`, `.svg`) relationship diagrams live in [`../../images/`](../../images/), and vector theme-adaptive SVGs live in [`../images/`](../images/).
""",
        encoding="utf-8",
    )

    (ROOT / "assets" / "README.md").write_text(
        """# Infinity-Context (`icx`) Branding & Platform Assets (`assets/`)

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
""",
        encoding="utf-8",
    )


def main() -> None:
    ensure_dirs()
    generate_images_and_icons()
    generate_graphviz_relationship_graphs()
    generate_mermaid_files_and_readmes()
    print("Successfully generated all images/, assets/, and docs/diagrams/ artifacts.")


if __name__ == "__main__":
    main()
