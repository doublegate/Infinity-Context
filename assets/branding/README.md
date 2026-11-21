# Branding status for the v1.2.0 candidate

The existing raster exports below are **archived concept artwork**, retained for provenance. Embedded compression ratios, provider cache-hit percentages, retrieval latency claims, packet examples and cross-agent capability labels are historical and non-normative. They do not describe the current runtime or qualify a release.

| Archived raster | Status |
| --- | --- |
| `icx-banner-full.png` | Original full concept artwork; embedded labels superseded |
| `icx-banner-1500x500.png` | Historical hero banner; embedded labels superseded |
| `icx-banner-1200x400.png` | Historical medium banner; embedded labels superseded |
| `icx-banner-900x300.png` | Historical compact banner; embedded labels superseded |
| `icx-social-preview-1280x640.png` | Historical raster social card; embedded labels superseded |

The [standalone vector social card](icx-social-preview-1280x640.svg) now uses contract labels instead of timing, compression or provider cache-hit claims. It is **not a pixel-equivalent source** for the archived raster card. Run `python3 scripts/generate_assets_and_graphs.py --branding-svg-only` to refresh only this SVG, without private JPG inputs or raster/logo changes.

Current behavior is defined in [ADR 0006](../../docs/adr/0006-v1-2-0-context-and-compatibility.md) and illustrated by the [current runtime diagrams](../../docs/diagrams/README.md). The managed context bound applies to icx-managed text; callers budget external content. Native Codex hooks capture output and cannot replace it. See [dated measurements and fixture limits](../../docs/numbers.md) for evidence; no branding image supplies a provider cache-hit, latency or model-quality guarantee.

Private source banner/logo JPGs are not present in this checkout. The five archived raster files above were preserved byte-for-byte; replacing their embedded artwork labels would require those source inputs or an explicitly chosen replacement design. The generator's overlay labels have been corrected so future generation does not reintroduce the stale quantitative claims. Original source artwork can itself contain historical labels and remains non-normative. Icons and logos are unchanged.
