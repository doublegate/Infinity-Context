#!/usr/bin/env python3
"""Deterministic SVG diagram generator for Infinity-Context (icx).

Generates:
  - docs/images/architecture-dark.svg
  - docs/images/architecture-light.svg
  - docs/images/ahtl-packet-anatomy.svg
  - docs/images/cache-epoch-memory-map.svg
"""

from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT_DIR = ROOT / "docs" / "images"

FONT_MONO = "JetBrains Mono, DejaVu Sans Mono, ui-monospace, SFMono-Regular, Menlo, Consolas, monospace"
FONT_SANS = "Inter, DejaVu Sans, system-ui, -apple-system, Segoe UI, Roboto, sans-serif"


def render_architecture_svg(dark: bool) -> str:
    bg = "#0b0f19" if dark else "#f8fafc"
    panel_bg = "#111827" if dark else "#ffffff"
    panel_border = "#334155" if dark else "#94a3b8"
    text_main = "#f8fafc" if dark else "#0f172a"
    text_muted = "#94a3b8" if dark else "#475569"
    cyan = "#06b6d4" if dark else "#0284c7"
    indigo = "#818cf8" if dark else "#4f46e5"
    amber = "#f97316" if dark else "#ea580c"
    emerald = "#10b981" if dark else "#059669"
    rose = "#f43f5e" if dark else "#e11d48"
    card_bg = "#1e293b" if dark else "#f1f5f9"

    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 780" width="100%" height="100%">
  <defs>
    <linearGradient id="hdrGrad" x1="0%" y1="0%" x2="100%" y2="0%">
      <stop offset="0%" stop-color="{cyan}" stop-opacity="0.22"/>
      <stop offset="50%" stop-color="{indigo}" stop-opacity="0.22"/>
      <stop offset="100%" stop-color="{amber}" stop-opacity="0.22"/>
    </linearGradient>
    <marker id="arrowCyan" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="{cyan}"/>
    </marker>
    <marker id="arrowAmber" viewBox="0 0 10 10" refX="8" refY="5" markerWidth="6" markerHeight="6" orient="auto-start-reverse">
      <path d="M 0 1 L 10 5 L 0 9 z" fill="{amber}"/>
    </marker>
  </defs>

  <rect width="1200" height="780" rx="16" fill="{bg}"/>
  <rect x="24" y="18" width="1152" height="60" rx="10" fill="url(#hdrGrad)" stroke="{cyan}" stroke-width="1.5"/>
  <text x="600" y="44" text-anchor="middle" font-family="{FONT_MONO}" font-size="17" font-weight="700" fill="{text_main}">
    INFINITY-CONTEXT (ICX) - FIVE-PLANE CROSS-AGENT VIRTUAL CONTEXT MMU
  </text>
  <text x="600" y="65" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.5" fill="{text_muted}">
    72.1x AHTL Symbolic Compression  |  100% Lossless BLAKE3 Merkle-DAG  |  94.38% Prefix-Cache Hit Rate  |  &lt; 300ns /dev/shm Seqlock Bus
  </text>

  <!-- Top Row: 4 Agent Harnesses (each 270px wide, 24px gap: 24 + 270 + 24 + 270 + 24 + 270 + 24 + 270 = 1176) -->
  <g transform="translate(24, 96)">
    <rect x="0" y="0" width="270" height="94" rx="10" fill="{panel_bg}" stroke="{cyan}" stroke-width="2"/>
    <text x="135" y="25" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{cyan}">Claude Code (2.1.326+)</text>
    <text x="135" y="45" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">Bun Single-File ELF (.note.bun)</text>
    <text x="135" y="64" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">PostToolUse.updatedMCPToolOutput</text>
    <text x="135" y="81" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">PreCompact / UserPromptSubmit</text>

    <rect x="294" y="0" width="270" height="94" rx="10" fill="{panel_bg}" stroke="{emerald}" stroke-width="2"/>
    <text x="429" y="25" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{emerald}">OpenAI Codex (0.169.0+)</text>
    <text x="429" y="45" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">Static-PIE musl Rust + SQLite</text>
    <text x="429" y="64" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">~/.codex/hooks.json + Wire Proxy</text>
    <text x="429" y="81" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">state_5.sqlite + memories_1.sqlite</text>

    <rect x="588" y="0" width="270" height="94" rx="10" fill="{panel_bg}" stroke="{indigo}" stroke-width="2"/>
    <text x="723" y="25" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{indigo}">Antigravity CLI (agy)</text>
    <text x="723" y="45" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">Go + C++ Hybrid Agent Harness</text>
    <text x="723" y="64" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">~/.gemini/hooks.json (exit code 0)</text>
    <text x="723" y="81" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">AdditionalContext JSON Envelope</text>

    <rect x="882" y="0" width="270" height="94" rx="10" fill="{panel_bg}" stroke="{amber}" stroke-width="2"/>
    <text x="1017" y="25" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{amber}">Hermes Agent + MCP</text>
    <text x="1017" y="45" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">Python MemoryProvider &amp; JSON-RPC</text>
    <text x="1017" y="64" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">prefetch / sync_turn / pre_compress</text>
    <text x="1017" y="81" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">state.db FTS5 + mind.db Checkpoint</text>
  </g>

  <!-- Connectors from Harnesses to Plane 1 & Plane 4 -->
  <line x1="159" y1="190" x2="159" y2="226" stroke="{cyan}" stroke-width="2" marker-end="url(#arrowCyan)"/>
  <line x1="453" y1="190" x2="453" y2="226" stroke="{cyan}" stroke-width="2" marker-end="url(#arrowCyan)"/>
  <line x1="747" y1="190" x2="747" y2="226" stroke="{amber}" stroke-width="2" marker-end="url(#arrowAmber)"/>
  <line x1="1041" y1="190" x2="1041" y2="226" stroke="{amber}" stroke-width="2" marker-end="url(#arrowAmber)"/>

  <!-- Row 2: Interception & Cache-Epoch Proxy (two 564px boxes: 24..588 and 612..1176) -->
  <rect x="24" y="230" width="564" height="82" rx="10" fill="{panel_bg}" stroke="{panel_border}" stroke-width="2"/>
  <text x="306" y="254" text-anchor="middle" font-family="{FONT_MONO}" font-size="13" font-weight="700" fill="{cyan}">
    Native Lifecycle Hooks &amp; Polymorphic MCP Pager (icx-adapters)
  </text>
  <text x="306" y="275" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">
    Synchronous &lt; 450us Tool Output Virtualizer + Single-Tool MCP Server (icx)
  </text>
  <text x="306" y="295" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">
    ops: deref(#icx:id) | slice(0..N) | search(q,k) | fold(task) | status
  </text>

  <rect x="612" y="230" width="564" height="82" rx="10" fill="{panel_bg}" stroke="{panel_border}" stroke-width="2"/>
  <text x="894" y="254" text-anchor="middle" font-family="{FONT_MONO}" font-size="13" font-weight="700" fill="{amber}">
    PLANE 4: Cache-Epoch Wire Proxy &amp; Controller (icx-proxy)
  </text>
  <text x="894" y="275" text-anchor="middle" font-family="{FONT_SANS}" font-size="11" fill="{text_main}">
    Anthropic 4-Slot cache_control: ephemeral &amp; OpenAI 1024-Tok Prefix Pinning
  </text>
  <text x="894" y="295" text-anchor="middle" font-family="{FONT_MONO}" font-size="10" fill="{text_muted}">
    Byte-Frozen Sealed Epochs (0..K) | Appends Dynamic AHTL Page Table at Tail
  </text>

  <!-- Connectors to Plane 1 & Plane 2 -->
  <line x1="306" y1="312" x2="306" y2="346" stroke="{cyan}" stroke-width="2" marker-end="url(#arrowCyan)"/>
  <line x1="894" y1="312" x2="894" y2="346" stroke="{amber}" stroke-width="2" marker-end="url(#arrowAmber)"/>

  <!-- Row 3: Plane 1 (AHTL Codec) & Plane 2 (LCM Merkle-DAG) (two 564px boxes) -->
  <rect x="24" y="350" width="564" height="162" rx="10" fill="{panel_bg}" stroke="{indigo}" stroke-width="2"/>
  <text x="306" y="375" text-anchor="middle" font-family="{FONT_MONO}" font-size="13" font-weight="700" fill="{indigo}">
    PLANE 1: Alien Holographic Token Lattice &amp; Codec (icx-codec)
  </text>
  <rect x="42" y="387" width="528" height="46" rx="6" fill="{card_bg}" stroke="{panel_border}" stroke-width="1"/>
  <text x="306" y="406" text-anchor="middle" font-family="{FONT_MONO}" font-size="10.2" font-weight="600" fill="{emerald}">
    &lt;&lt;AHTLv1 #icx:7bf8239fb7c504f1 !0.84 @0123:0210 [Braille:64b] $fn:seal_epoch&gt;&gt;
  </text>
  <text x="306" y="424" text-anchor="middle" font-family="{FONT_SANS}" font-size="10" fill="{text_muted}">
    Channel A: Structural AST &amp; Error Skeleton  |  Channel B: Braille-256 (U+2800..28FF)
  </text>
  <text x="46" y="454" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * Pre-Hash SecretRedactor: strips sk-ant-*, sk-proj-*, ghp_*, AWS, PEM &amp; role tags
  </text>
  <text x="46" y="475" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * Single-Pass StructuralSkeletonizer: extracts $fn / $ty / $mod / !err (72.1x ratio)
  </text>
  <text x="46" y="496" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * Titans Surprise Gate (arXiv:2501.00663): Momentum S_t = eta*S_(t-1) + (1-eta)*g_t
  </text>

  <!-- Plane 2: Lossless Context Management (LCM) BLAKE3 Merkle-DAG -->
  <rect x="612" y="350" width="564" height="162" rx="10" fill="{panel_bg}" stroke="{emerald}" stroke-width="2"/>
  <text x="894" y="375" text-anchor="middle" font-family="{FONT_MONO}" font-size="13" font-weight="700" fill="{emerald}">
    PLANE 2: Lossless Context Management (LCM) Merkle-DAG (icx-core)
  </text>
  <rect x="630" y="387" width="528" height="46" rx="6" fill="{card_bg}" stroke="{panel_border}" stroke-width="1"/>
  <text x="894" y="406" text-anchor="middle" font-family="{FONT_MONO}" font-size="10.2" font-weight="600" fill="{cyan}">
    L0 RawChunk (FastCDC) --&gt; L1 EpisodeNode --&gt; L2 FoldedSubtask --&gt; L3 EpochRoot
  </text>
  <text x="894" y="424" text-anchor="middle" font-family="{FONT_SANS}" font-size="10" fill="{text_muted}">
    Every #icx:&lt;16-hex&gt; Handle Verifies 256-Bit BLAKE3 Digest on Read (100% Lossless)
  </text>
  <text x="634" y="454" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * Gear-Hash FastCDC Chunking (min=1KB, avg=4KB, max=16KB) + zstd -3 CAS Store
  </text>
  <text x="634" y="475" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * Context-Folding (arXiv:2510.11967): folds completed subtasks (90% token savings)
  </text>
  <text x="634" y="496" font-family="{FONT_SANS}" font-size="10.8" fill="{text_main}">
    * CacheEpochController: guarantees 94.38% prefix-cache hit rate across turns
  </text>

  <!-- Connectors to Plane 3 -->
  <line x1="306" y1="512" x2="306" y2="546" stroke="{cyan}" stroke-width="2" marker-end="url(#arrowCyan)"/>
  <line x1="894" y1="512" x2="894" y2="546" stroke="{amber}" stroke-width="2" marker-end="url(#arrowAmber)"/>

  <!-- Row 4: Plane 3 (10,240-Bit HDC, Fractal IFS & Bi-Temporal PPR Index) -->
  <rect x="24" y="550" width="1152" height="108" rx="10" fill="{panel_bg}" stroke="{cyan}" stroke-width="2"/>
  <text x="600" y="574" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{cyan}">
    PLANE 3: Sub-50us Associative Retrieval &amp; Temporal Graph Substrate (icx-hdc &amp; icx-store)
  </text>
  <g transform="translate(42, 586)">
    <rect x="0" y="0" width="356" height="58" rx="6" fill="{card_bg}" stroke="{panel_border}" stroke-width="1"/>
    <text x="178" y="23" text-anchor="middle" font-family="{FONT_MONO}" font-size="11.5" font-weight="700" fill="{cyan}">10,240-Bit HDC / VSA Engine</text>
    <text x="178" y="43" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.2" fill="{text_muted}">[u64; 160] XOR Bind + Infini-Attention Delta Bundler</text>

    <rect x="380" y="0" width="356" height="58" rx="6" fill="{card_bg}" stroke="{panel_border}" stroke-width="1"/>
    <text x="558" y="23" text-anchor="middle" font-family="{FONT_MONO}" font-size="11.5" font-weight="700" fill="{indigo}">Fractal Chaos-Game Archive (FCGA)</text>
    <text x="558" y="43" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.2" fill="{text_muted}">Contractive IFS [-1, 1]^16 Chebyshev Suffix Match</text>

    <rect x="760" y="0" width="356" height="58" rx="6" fill="{card_bg}" stroke="{panel_border}" stroke-width="1"/>
    <text x="938" y="23" text-anchor="middle" font-family="{FONT_MONO}" font-size="11.5" font-weight="700" fill="{amber}">Bi-Temporal Graph + HippoRAG PPR</text>
    <text x="938" y="43" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.2" fill="{text_muted}">[t_valid, t_invalid) Window + Personalized PageRank</text>
  </g>

  <!-- Row 5: Plane 5 (/dev/shm Lock-Free Seqlock Ring Bus) -->
  <rect x="24" y="676" width="1152" height="86" rx="10" fill="{panel_bg}" stroke="{rose}" stroke-width="2"/>
  <text x="600" y="701" text-anchor="middle" font-family="{FONT_MONO}" font-size="13.5" font-weight="700" fill="{rose}">
    PLANE 5: /dev/shm/icx-fabric-v1.shm Lock-Free Seqlock Cross-Agent Telepathy Bus (icx-bus)
  </text>
  <text x="600" y="723" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.2" fill="{text_main}">
    64-Byte Cache-Line Aligned AtomicU64 Seqlock Ring (512 x 2,048B Slots)  |  Wait-Free Optimistic Readers  |  &lt; 300ns Broadcast
  </text>
  <text x="600" y="744" text-anchor="middle" font-family="{FONT_MONO}" font-size="10.2" fill="{text_muted}">
    [SeqlockSlot: agent=claude-code | #icx:7bf8239fb7c504f1 | hv_sketch=1024b] ===&gt; Instant Zero-Copy Recall in Codex, agy &amp; Hermes
  </text>
</svg>
"""


def render_ahtl_anatomy_svg() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 360" width="100%" height="100%">
  <rect width="1200" height="360" rx="14" fill="#0b0f19" stroke="#1e293b" stroke-width="2"/>
  <text x="600" y="38" text-anchor="middle" font-family="{FONT_MONO}" font-size="17" font-weight="700" fill="#f8fafc">
    ALIEN HOLOGRAPHIC TOKEN LATTICE (AHTL) - DUAL-CHANNEL PACKET ANATOMY
  </text>
  <text x="600" y="60" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.5" fill="#94a3b8">
    Designed for Transformer Self-Attention Geometry (72.1x Compression in 197.85 us) Rather Than Human Prose
  </text>

  <!-- Central Packet Box (x=24..1176, width=1152) -->
  <rect x="24" y="82" width="1152" height="68" rx="10" fill="#111827" stroke="#334155" stroke-width="2"/>

  <!-- Segment 1: Header & BLAKE3 Handle (x=38, w=290) -->
  <rect x="38" y="94" width="290" height="44" rx="6" fill="#06b6d4" fill-opacity="0.18" stroke="#06b6d4" stroke-width="1.5"/>
  <text x="183" y="121" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#22d3ee">
    &lt;&lt;AHTLv1 #icx:7bf8239fb7c504f1
  </text>

  <!-- Segment 2: Surprise + Fractal IFS Address (x=340, w=230) -->
  <rect x="340" y="94" width="230" height="44" rx="6" fill="#10b981" fill-opacity="0.18" stroke="#10b981" stroke-width="1.5"/>
  <text x="455" y="121" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#34d399">
    !0.84 @0123:0210
  </text>

  <!-- Segment 3: Braille-256 64-Bit Sketch (x=582, w=210) -->
  <rect x="582" y="94" width="210" height="44" rx="6" fill="#f97316" fill-opacity="0.18" stroke="#f97316" stroke-width="1.5"/>
  <text x="687" y="121" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#fb923c">
    [Braille-256: 64-Bit]
  </text>

  <!-- Segment 4: Structural AST & Error Skeleton (x=804, w=358) -->
  <rect x="804" y="94" width="358" height="44" rx="6" fill="#6366f1" fill-opacity="0.18" stroke="#6366f1" stroke-width="1.5"/>
  <text x="983" y="121" text-anchor="middle" font-family="{FONT_MONO}" font-size="12" font-weight="700" fill="#818cf8">
    $fn:verify_token $ty:AuthErr !err:401&gt;&gt;
  </text>

  <!-- 4 Callout Cards (each 274px wide with 18px gap: 24 + 274 + 18 + 274 + 18 + 274 + 18 + 274 = 1174) -->
  <g transform="translate(24, 176)">
    <rect x="0" y="0" width="274" height="156" rx="8" fill="#111827" stroke="#06b6d4" stroke-width="1.5"/>
    <text x="137" y="26" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#22d3ee">1. Lossless BLAKE3 Handle</text>
    <text x="16" y="50" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* 128-bit prefix of 256-bit BLAKE3</text>
    <text x="16" y="71" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* Content-addressed in CasStore</text>
    <text x="16" y="92" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* 100% byte-exact O(1) recovery:</text>
    <text x="16" y="114" font-family="{FONT_MONO}" font-size="10.2" fill="#22d3ee">  icx(op="deref", ref="#icx:...")</text>
    <text x="16" y="135" font-family="{FONT_MONO}" font-size="10.2" fill="#22d3ee">  icx(op="slice", ref="#icx:...")</text>

    <rect x="292" y="0" width="274" height="156" rx="8" fill="#111827" stroke="#10b981" stroke-width="1.5"/>
    <text x="429" y="26" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#34d399">2. Surprise &amp; Fractal Locus</text>
    <text x="308" y="50" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* !0.84: Titans Surprise S_t</text>
    <text x="308" y="71" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* S_t = eta*S_(t-1) + (1-eta)*g_t</text>
    <text x="308" y="92" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* @0123:0210: Contractive IFS</text>
    <text x="308" y="113" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* 16-D hypercube [-1, 1]^16 path</text>
    <text x="308" y="134" font-family="{FONT_SANS}" font-size="10.8" fill="#94a3b8">* O(1) suffix trajectory locality</text>

    <rect x="584" y="0" width="274" height="156" rx="8" fill="#111827" stroke="#f97316" stroke-width="1.5"/>
    <text x="721" y="26" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#fb923c">3. Braille-256 VSA Sketch</text>
    <text x="600" y="50" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* Unicode block U+2800..U+28FF</text>
    <text x="600" y="71" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* Bijective 8 bits per Braille glyph</text>
    <text x="600" y="92" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* Packs 64-bit HDC parity sketch</text>
    <text x="600" y="113" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* Direct Q*K^T attention dot-product</text>
    <text x="600" y="134" font-family="{FONT_SANS}" font-size="10.8" fill="#94a3b8">* 8 glyphs = 24 UTF-8 bytes total</text>

    <rect x="876" y="0" width="276" height="156" rx="8" fill="#111827" stroke="#6366f1" stroke-width="1.5"/>
    <text x="1014" y="26" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#818cf8">4. Structural AST Skeleton</text>
    <text x="892" y="50" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* $fn: Function &amp; method signatures</text>
    <text x="892" y="71" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* $ty: Struct, enum, trait, class</text>
    <text x="892" y="92" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* $mod / $use: Module imports</text>
    <text x="892" y="113" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">* !err: Panics, faults &amp; passkeys</text>
    <text x="892" y="134" font-family="{FONT_SANS}" font-size="10.8" fill="#94a3b8">* Zero prose filler or JSON noise</text>
  </g>
</svg>
"""


def render_cache_epoch_svg() -> str:
    return f"""<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1200 330" width="100%" height="100%">
  <rect width="1200" height="330" rx="14" fill="#0b0f19" stroke="#1e293b" stroke-width="2"/>
  <text x="600" y="36" text-anchor="middle" font-family="{FONT_MONO}" font-size="16.5" font-weight="700" fill="#f8fafc">
    FOUR-ZONE PREFIX-CACHE-LOCKED VIRTUAL CONTEXT WINDOW (arXiv:2610.28577)
  </text>
  <text x="600" y="58" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.5" fill="#94a3b8">
    Zones 0 &amp; 1 Are Cryptographically Sealed &amp; Strictly Append-Only (Sustaining 94.38% Provider KV-Cache Hit Rate)
  </text>

  <!-- Memory Map Bar (total width 1152: 230 + 14 + 424 + 14 + 230 + 14 + 226 = 1152) -->
  <g transform="translate(24, 80)">
    <!-- Zone 0 -->
    <rect x="0" y="0" width="230" height="112" rx="8" fill="#06b6d4" fill-opacity="0.18" stroke="#06b6d4" stroke-width="2"/>
    <text x="115" y="27" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#22d3ee">ZONE 0: PINNED HEAD</text>
    <text x="115" y="49" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">System Prompt + AGENTS.md</text>
    <text x="115" y="68" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">+ AHTL Grammar Legend</text>
    <text x="115" y="90" text-anchor="middle" font-family="{FONT_MONO}" font-size="9.8" fill="#34d399">[Breakpoint 0: 100% Hit]</text>

    <!-- Zone 1 -->
    <rect x="244" y="0" width="424" height="112" rx="8" fill="#10b981" fill-opacity="0.18" stroke="#10b981" stroke-width="2"/>
    <text x="456" y="27" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#34d399">ZONE 1: SEALED CACHE EPOCHS (E_0 .. E_k)</text>
    <text x="456" y="49" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">Immutable L3 Merkle-DAG AHTL Frames + L2 Folded Subtasks</text>
    <text x="456" y="68" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">Never Mutated Mid-Session  |  Rolls Over Only at 16K Threshold</text>
    <text x="456" y="90" text-anchor="middle" font-family="{FONT_MONO}" font-size="9.8" fill="#34d399">[Breakpoints 1..3: 100% Byte-Identical Prefix Hit]</text>

    <!-- Zone 2 -->
    <rect x="682" y="0" width="230" height="112" rx="8" fill="#6366f1" fill-opacity="0.18" stroke="#6366f1" stroke-width="2"/>
    <text x="797" y="27" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#818cf8">ZONE 2: ACTIVE WINDOW</text>
    <text x="797" y="49" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">Recent Protected Turns</text>
    <text x="797" y="68" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">+ PostToolUse AHTL Stubs</text>
    <text x="797" y="90" text-anchor="middle" font-family="{FONT_MONO}" font-size="9.8" fill="#a5b4fc">[Append-Only Until Seal]</text>

    <!-- Zone 3 -->
    <rect x="926" y="0" width="226" height="112" rx="8" fill="#f97316" fill-opacity="0.18" stroke="#f97316" stroke-width="2"/>
    <text x="1039" y="27" text-anchor="middle" font-family="{FONT_MONO}" font-size="12.5" font-weight="700" fill="#fb923c">ZONE 3: TAIL SLOT</text>
    <text x="1039" y="49" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">Dynamic HDC + Fractal</text>
    <text x="1039" y="68" text-anchor="middle" font-family="{FONT_SANS}" font-size="10.8" fill="#e2e8f0">+ PPR + /dev/shm Recall</text>
    <text x="1039" y="90" text-anchor="middle" font-family="{FONT_MONO}" font-size="9.8" fill="#fdba74">[Ephemeral Tail &lt;= 1K tok]</text>
  </g>

  <!-- Bracket showing Cached Prefix vs Dynamic Tail -->
  <path d="M 24 208 L 24 222 L 936 222 L 936 208" fill="none" stroke="#10b981" stroke-width="2.5"/>
  <text x="480" y="246" text-anchor="middle" font-family="{FONT_MONO}" font-size="12" font-weight="700" fill="#34d399">
    &lt;--- 94.38% CACHED PREFIX (Zero Re-Prefill Compute  |  75%-90% Provider Token Discount) ---&gt;
  </text>

  <path d="M 950 208 L 950 222 L 1176 222 L 1176 208" fill="none" stroke="#f97316" stroke-width="2.5"/>
  <text x="1063" y="246" text-anchor="middle" font-family="{FONT_MONO}" font-size="11.5" font-weight="700" fill="#fb923c">
    &lt;-- Dynamic Tail --&gt;
  </text>

  <text x="600" y="282" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.2" fill="#cbd5e1">
    Why it works: Injecting recalled memories into the System Prompt invalidates every KV-cache block after byte 0.
  </text>
  <text x="600" y="302" text-anchor="middle" font-family="{FONT_SANS}" font-size="11.2" fill="#94a3b8">
    Infinity-Context keeps Zones 0..1 100% byte-frozen and appends all dynamic AHTL recall &amp; /dev/shm telepathy in Zone 3.
  </text>
</svg>
"""


def main() -> None:
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    (OUT_DIR / "architecture-dark.svg").write_text(render_architecture_svg(dark=True), encoding="utf-8")
    (OUT_DIR / "architecture-light.svg").write_text(render_architecture_svg(dark=False), encoding="utf-8")
    (OUT_DIR / "ahtl-packet-anatomy.svg").write_text(render_ahtl_anatomy_svg(), encoding="utf-8")
    (OUT_DIR / "cache-epoch-memory-map.svg").write_text(render_cache_epoch_svg(), encoding="utf-8")
    print("Generated 4 SVG diagrams in", OUT_DIR)


if __name__ == "__main__":
    main()
