#!/usr/bin/env python3
"""Render current runtime SVG/PNG summaries and identical mirrors under images/.

Requires rsvg-convert for PNGs. No branding assets or private inputs are used.
"""
from html import escape
from pathlib import Path
import hashlib
import subprocess

ROOT = Path(__file__).resolve().parent.parent
FONT = "DejaVu Sans, sans-serif"


def diagram(title: str, subtitle: str, cards: list, dark: bool = True) -> str:
    """Render a fixed two-column grid with plain, selectable SVG text."""
    bg, panel, text, muted = ("#0b0f19", "#111827", "#f8fafc", "#b4c0d0") if dark else ("#f8fafc", "#ffffff", "#0f172a", "#475569")
    rows = (len(cards) + 1) // 2
    height = 144 + rows * 160
    digest = hashlib.sha256(Path(__file__).read_bytes()).hexdigest()
    parts = [f'<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="{height}" viewBox="0 0 1200 {height}" role="img">',
             f'<title>{escape(title)}</title>',
             f'<desc>{escape(subtitle)}</desc>',
             f'<metadata>runtime=v1.2.0-candidate; generator=scripts/render_diagrams.py; sha256={digest}; measurements=docs/numbers.md</metadata>',
             f'<rect width="1200" height="{height}" rx="16" fill="{bg}"/>',
             f'<g font-family="{FONT}">',
             f'<text x="32" y="40" fill="{text}" font-size="23" font-weight="700">{escape(title)}</text>',
             f'<text x="32" y="68" fill="{muted}" font-size="14">{escape(subtitle)}</text>']
    colors = ["#38bdf8", "#a78bfa", "#34d399", "#fb923c", "#f472b6", "#22d3ee"]
    for n, (heading, lines) in enumerate(cards):
        x, y = 32 + (n % 2) * 584, 94 + (n // 2) * 160
        color = colors[n % len(colors)]
        parts += [f'<rect x="{x}" y="{y}" width="552" height="142" rx="10" fill="{panel}" stroke="{color}" stroke-width="2"/>',
                  f'<text x="{x+18}" y="{y+29}" fill="{color}" font-size="17" font-weight="700">{escape(heading)}</text>']
        for k, line in enumerate(lines):
            parts.append(f'<text x="{x+18}" y="{y+54+k*21}" fill="{text}" font-size="14">{escape(line)}</text>')
    parts += [f'<text x="32" y="{height-22}" fill="{muted}" font-size="12">v1.2.0 candidate | Contracts: docs/adr/0006-v1-2-0-context-and-compatibility.md | Evidence: docs/numbers.md</text>', '</g></svg>']
    return "\n".join(parts) + "\n"


def architecture(dark: bool) -> str:
    return diagram("Infinity-Context: five-plane runtime", "Capture → redact → durable storage → hybrid recall → budgeted managed context", [
        ("Agent surfaces / validated MMU", ["Claude: supported native output replacement", "Codex PostToolUse: capture only; capability false", "agy context hooks / Hermes ContextEngine shim", "MCP: canonical icx; strict envelope and arguments"]),
        ("Plane 1: symbolic AHTL codec", ["Structural skeleton + Braille projection", "Lossy model-facing view; no model-quality guarantee", "Redact before CAS hashing and persistence", "Exact post-redaction source remains recoverable"]),
        ("Plane 2: BLAKE3 CAS / Merkle-DAG", ["FastCDC chunks and verified content-addressed reads", "Immutable sealed history; selection is budgeted", "Daemon: WAL synchronous=FULL", "Commit before successful ACK; no write-behind"]),
        ("Plane 3: lexical-first hybrid recall", ["Exact-token postings + HDC / fractal candidates", "Lexical coverage, combined score, then artifact ID", "ICXHDC04 writes; 03 reads + MMU postings rebuild", "Numeric validation; provenance checked separately"]),
        ("Plane 4: bounded managed context", ["Head + selected sealed frames + recent frames + tail", "Default 32768 - 4096 completion - 4096 external", "24576 UTF-8 bytes; caller budgets external content", "HTTP/1 JSON proxy; no provider cache-hit promise"]),
        ("Plane 5: cross-agent bus", ["In-process atomics; file-backed positioned I/O", "File writer lock; sequence checks and loss signalling", "Tail keeps useful whole frames and compact handles", "Unsent bus events remain queued for a later turn"]),
    ], dark)


def anatomy() -> str:
    return diagram("AHTL Profile A: field map", "Symbolic wire fields and projected sketches; placeholders below are not a literal packet.", [
        ("1. Header and artifact reference", ["⟦AHTL v1│#icx:<12-or-16-hex>│…⟧", "48-bit or 64-bit prefix; full BLAKE3 digest verified", "Canonical pager operation: deref with id", "Recovery work scales with returned bytes"]),
        ("2. Coordinate and digest", ["│fc:<64-hex>│hv:<16-hex>│", "Fractal coordinates: signed Q15, scale 32767", "Digest/sketch fields are lossy summaries", "Surprise metadata is not this Profile A field map"]),
        ("3. Braille projection", ["Profile A: 64 glyphs = 192 UTF-8 bytes", "512 projected bits; one byte per Braille glyph", "Profile B: 8 glyphs = 24 UTF-8 bytes", "No claim of learned attention semantics"]),
        ("4. Structural skeleton", ["Symbolic clauses preserve selected structure", "Example clause: ⊕fn:verify_token⋮¬unwrap()", "Skeleton and projected sketch are lossy", "Full post-redaction source is available through CAS"]),
    ])


def epoch() -> str:
    return diagram("Managed context: four budgeted zones", "Immutable cold storage does not require every historical frame to appear in every emitted window.", [
        ("Zone 0: head", ["Caller-provided head and AHTL legend", "Included in the managed allowance", "External instructions/current message/tools:", "caller reserves enough external allowance"]),
        ("Zone 1: selected sealed epochs", ["Cold epoch bytes remain immutable", "Select only frames that fit the configured policy", "Paging or policy changes may alter emitted prefix", "Provider cache-hit rates are not guaranteed"]),
        ("Zone 2: recent active frames", ["Prioritize useful whole recent frames", "Older detail remains recoverable through CAS", "Epoch sealing threshold is configurable", "Default epoch threshold: 4096 conservative units"]),
        ("Zone 3: structured tail", ["Header / best recall, then bus and PPR frames", "Default cap: 1024 conservative units", "Compact handles; no arbitrary byte-cut frames", "Unsent bus events remain pending"]),
        ("Default managed allowance", ["32768 context - 4096 completion - 4096 external", "= 24576 UTF-8 bytes for managed text", "Estimator: utf8-bytes-upper-bound-v1", "One UTF-8 byte per conservative token allowance"]),
        ("Boundary of this guarantee", ["The selected managed text is bounded", "The caller budgets all external request content", "Native host transcripts can retain original output", "No whole-provider-request or billing guarantee"]),
    ])


def main() -> None:
    output = ROOT / "docs" / "images"
    output.mkdir(parents=True, exist_ok=True)
    mirror = ROOT / "images"
    mirror.mkdir(parents=True, exist_ok=True)
    illustrations = {"architecture-dark": architecture(True), "architecture-light": architecture(False),
                     "ahtl-packet-anatomy": anatomy(), "cache-epoch-memory-map": epoch()}
    for name, svg in illustrations.items():
        path = output / f"{name}.svg"
        path.write_text(svg, encoding="utf-8")
        subprocess.run(["rsvg-convert", "-z", "2", str(path), "-o", str(path.with_suffix(".png"))], check=True)
        for suffix in (".svg", ".png"):
            (mirror / f"{name}{suffix}").write_bytes(path.with_suffix(suffix).read_bytes())
    print("Generated 4 SVG/PNG diagrams and identical images/ mirrors.")


if __name__ == "__main__":
    main()
