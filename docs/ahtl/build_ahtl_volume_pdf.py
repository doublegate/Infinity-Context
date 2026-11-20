#!/usr/bin/env python3
"""
Reproducible Master Technical Volume PDF Builder for AHTL v1 (icx v1.1.0):
Generates `docs/ahtl/AHTL_v1_Complete_Technical_Specification_Volume.pdf`
from the 9 Markdown specification files and visual assets in `docs/ahtl/`.
"""

from pathlib import Path
import re
import textwrap
import markdown
import weasyprint

AHTL_DIR = Path(__file__).resolve().parent
OUT_PDF = AHTL_DIR / "AHTL_v1_Complete_Technical_Specification_Volume.pdf"

CHAPTER_FILES = [
    ("EXEC-00", "Executive Overview & Master Architecture", "README.md", "#00E5FF"),
    ("CH-01", "Chapter 01 — Formal Grammar, Wire Profiles & Packet Specification", "01-formal-grammar-and-packet-specification.md", "#00E5FF"),
    ("CH-02", "Chapter 02 — Braille-256 Spectral Bijection & Topological Hologram", "02-braille-256-spectral-bijection.md", "#F59E0B"),
    ("CH-03", "Chapter 03 — Contractive IFS & Fractal Chaos-Game Trajectory Coordinates", "03-fractal-ifs-chaos-game-coordinates.md", "#10B981"),
    ("CH-04", "Chapter 04 — 10,240-Bit Hyperdimensional Lattice & Associative Algebra", "04-hyperdimensional-10240-bit-lattice.md", "#8B5CF6"),
    ("CH-05", "Chapter 05 — Structural AST Skeletonization & Secret Redaction", "05-structural-ast-skeletonization-and-redaction.md", "#F43F5E"),
    ("CH-06", "Chapter 06 — BLAKE3 Merkle-DAG CAS & Virtual Memory Handles", "06-blake3-merkle-cas-and-virtual-handles.md", "#0284C7"),
    ("CH-07", "Chapter 07 — Transformer Self-Attention, BPE Mechanics & Cache Epochs", "07-transformer-attention-and-bpe-mechanics.md", "#7C3AED"),
    ("CH-08", "Chapter 08 — Reference Implementation Blueprint & Verification Suite", "08-v1-reference-implementation-blueprint.md", "#059669"),
]

RAW_LATEX_SYMBOLS = [
    (r"\Longleftrightarrow", "⟺"),
    (r"\iff", "⟺"),
    (r"\Longrightarrow", "⟹"),
    (r"\implies", "⟹"),
    (r"\longrightarrow", "⟶"),
    (r"\leftarrow", "←"),
    (r"\mapsto", "↦"),
    (r"\to", "→"),
    (r"\bigoplus", "⨁"),
    (r"\bigcup", "⋃"),
    (r"\bigcap", "⋂"),
    (r"\sum", "∑"),
    (r"\prod", "∏"),
    (r"\oplus", "⊕"),
    (r"\ominus", "⊖"),
    (r"\otimes", "⊗"),
    (r"\odot", "⊙"),
    (r"\times", "×"),
    (r"\cdot", "·"),
    (r"\circ", "∘"),
    (r"\perp", "⊥"),
    (r"\rtimes", "⋊"),
    (r"\wr", "≀"),
    (r"\square", "□"),
    (r"\blacksquare", "■"),
    (r"\leq", "≤"),
    (r"\le", "≤"),
    (r"\geq", "≥"),
    (r"\ge", "≥"),
    (r"\neq", "≠"),
    (r"\ne", "≠"),
    (r"\approx", "≈"),
    (r"\equiv", "≡"),
    (r"\cong", "≅"),
    (r"\sim", "∼"),
    (r"\ggg", "⋙"),
    (r"\ll", "≪"),
    (r"\gg", "≫"),
    (r"\infty", "∞"),
    (r"\notin", "∉"),
    (r"\in", "∈"),
    (r"\subseteq", "⊆"),
    (r"\subset", "⊂"),
    (r"\setminus", "∖"),
    (r"\cup", "∪"),
    (r"\cap", "∩"),
    (r"\emptyset", "∅"),
    (r"\forall", "∀"),
    (r"\exists", "∃"),
    (r"\land", "∧"),
    (r"\lor", "∨"),
    (r"\neg", "¬"),
    (r"\partial", "∂"),
    (r"\lfloor", "⌊"),
    (r"\rfloor", "⌋"),
    (r"\lceil", "⌈"),
    (r"\rceil", "⌉"),
    (r"\langle", "⟨"),
    (r"\rangle", "⟩"),
    (r"\Vert", "‖"),
    (r"\|", "‖"),
    (r"\mid", " | "),
    (r"\cdots", "⋯"),
    (r"\dots", "…"),
    (r"\pmod", " mod "),
    (r"\bmod", " mod "),
    (r"\pm", "±"),
    (r"\left", " "),
    (r"\right", " "),
    (r"\Bigg", " "),
    (r"\bigg", " "),
    (r"\Big", " "),
    (r"\big", " "),
    (r"\log", '<span class="math-op">log</span>'),
    (r"\ln", '<span class="math-op">ln</span>'),
    (r"\exp", '<span class="math-op">exp</span>'),
    (r"\max", '<span class="math-op">max</span>'),
    (r"\min", '<span class="math-op">min</span>'),
    (r"\lim", '<span class="math-op">lim</span>'),
    (r"\sin", '<span class="math-op">sin</span>'),
    (r"\cos", '<span class="math-op">cos</span>'),
    (r"\mathbb{E}", "𝔼"),
    (r"\mathbb{P}", "ℙ"),
    (r"\mathbb{R}", "ℝ"),
    (r"\mathbb{Z}", "ℤ"),
    (r"\mathbb{C}", "ℂ"),
    (r"\mathbb{I}", "𝕀"),
    (r"\mathcal{X}", "𝒳"),
    (r"\mathcal{F}", "ℱ"),
    (r"\mathcal{W}", "𝒲"),
    (r"\mathcal{H}", "ℋ"),
    (r"\mathcal{S}", "𝒮"),
    (r"\mathcal{N}", "𝒩"),
    (r"\mathcal{T}", "𝒯"),
    (r"\mathcal{M}", "ℳ"),
    (r"\mathcal{P}", "𝒫"),
    (r"\varepsilon", "ε"),
    (r"\alpha", "α"),
    (r"\beta", "β"),
    (r"\gamma", "γ"),
    (r"\delta", "δ"),
    (r"\epsilon", "ε"),
    (r"\zeta", "ζ"),
    (r"\eta", "η"),
    (r"\theta", "θ"),
    (r"\kappa", "κ"),
    (r"\lambda", "λ"),
    (r"\mu", "μ"),
    (r"\nu", "ν"),
    (r"\xi", "ξ"),
    (r"\pi", "π"),
    (r"\rho", "ρ"),
    (r"\sigma", "σ"),
    (r"\tau", "τ"),
    (r"\phi", "φ"),
    (r"\chi", "χ"),
    (r"\psi", "ψ"),
    (r"\omega", "ω"),
    (r"\Delta", "Δ"),
    (r"\Theta", "Θ"),
    (r"\Lambda", "Λ"),
    (r"\Xi", "Ξ"),
    (r"\Pi", "Π"),
    (r"\Sigma", "Σ"),
    (r"\Phi", "Φ"),
    (r"\Psi", "Ψ"),
    (r"\Omega", "Ω"),
]

# Sort longest command first so prefixes like \in or \subset never shadow \infty or \subseteq
LATEX_SYMBOLS = sorted(RAW_LATEX_SYMBOLS, key=lambda kv: len(kv[0]), reverse=True)


def find_balanced_braces(text: str, start_idx: int) -> tuple[str, int] | None:
    if start_idx >= len(text) or text[start_idx] != "{":
        return None
    depth = 0
    for i in range(start_idx, len(text)):
        if text[i] == "{":
            depth += 1
        elif text[i] == "}":
            depth -= 1
            if depth == 0:
                return text[start_idx + 1 : i], i + 1
    return None


def replace_cmd_1arg(text: str, cmd: str, fmt_fn) -> str:
    while True:
        idx = text.find(cmd + "{")
        if idx == -1:
            break
        res = find_balanced_braces(text, idx + len(cmd))
        if not res:
            break
        inner, end_idx = res
        text = text[:idx] + fmt_fn(inner) + text[end_idx:]
    return text


def replace_cmd_2arg(text: str, cmd: str, fmt_fn) -> str:
    while True:
        idx = text.find(cmd + "{")
        if idx == -1:
            break
        res1 = find_balanced_braces(text, idx + len(cmd))
        if not res1:
            break
        arg1, end1 = res1
        while end1 < len(text) and text[end1].isspace():
            end1 += 1
        res2 = find_balanced_braces(text, end1)
        if not res2:
            break
        arg2, end2 = res2
        text = text[:idx] + fmt_fn(arg1, arg2) + text[end2:]
    return text


def replace_underbrace(text: str) -> str:
    cmd = r"\underbrace"
    while True:
        idx = text.find(cmd + "{")
        if idx == -1:
            break
        res1 = find_balanced_braces(text, idx + len(cmd))
        if not res1:
            break
        arg1, end1 = res1
        while end1 < len(text) and text[end1].isspace():
            end1 += 1
        if end1 < len(text) and text[end1] == "_":
            end1 += 1
            while end1 < len(text) and text[end1].isspace():
                end1 += 1
            res2 = find_balanced_braces(text, end1)
            if res2:
                arg2, end2 = res2
                rep = f'<span class="underbrace">({clean_latex_expr(arg1)})<sub>[{clean_latex_expr(arg2)}]</sub></span>'
                text = text[:idx] + rep + text[end2:]
                continue
        rep = f"({clean_latex_expr(arg1)})"
        text = text[:idx] + rep + text[end1:]
    return text


def clean_latex_expr(expr: str) -> str:
    s = expr.strip()

    # Handle \begin{pmatrix} ... \end{pmatrix}
    def repl_pmatrix(m):
        body = m.group(1).strip()
        rows = [r.strip() for r in body.split(r"\\") if r.strip()]
        row_html = []
        for r in rows:
            cells = [clean_latex_expr(c.strip()) for c in re.split(r"(?<!\\)&", r)]
            tds = "".join(f"<td>{c}</td>" for c in cells)
            row_html.append(f"<tr>{tds}</tr>")
        return f'<span class="matrix-wrap"><table class="math-matrix">{"".join(row_html)}</table></span>'

    s = re.sub(r"\\begin\{pmatrix\}(.*?)\\end\{pmatrix\}", repl_pmatrix, s, flags=re.DOTALL)

    # Handle \begin{cases} ... \end{cases}
    def repl_cases(m):
        body = m.group(1).strip()
        rows = [r.strip() for r in body.split(r"\\") if r.strip()]
        row_html = []
        for r in rows:
            cells = [clean_latex_expr(c.strip()) for c in re.split(r"(?<!\\)&", r)]
            tds = "".join(f"<td>{c}</td>" for c in cells)
            row_html.append(f"<tr>{tds}</tr>")
        return f'<span class="cases-wrap"><table class="math-cases">{"".join(row_html)}</table></span>'

    s = re.sub(r"\\begin\{cases\}(.*?)\\end\{cases\}", repl_cases, s, flags=re.DOTALL)

    # Handle \begin{aligned} ... \end{aligned}
    def repl_aligned(m):
        body = m.group(1).strip()
        rows = [r.strip() for r in body.split(r"\\") if r.strip()]
        lines = []
        for r in rows:
            cleaned = " ".join(clean_latex_expr(c.strip()) for c in re.split(r"(?<!\\)&", r) if c.strip())
            lines.append(f'<div class="math-aligned-row">{cleaned}</div>')
        return "".join(lines)

    s = re.sub(r"\\begin\{aligned\}(.*?)\\end\{aligned\}", repl_aligned, s, flags=re.DOTALL)

    # Escape literal braces, ampersands, percent, and underscores BEFORE subscript matching
    s = (
        s.replace(r"\{", "&#123;")
        .replace(r"\}", "&#125;")
        .replace(r"\&", "&amp;")
        .replace(r"\%", "%")
        .replace(r"\_", "&#95;")
    )

    # Handle \underbrace{A}_{B}
    s = replace_underbrace(s)

    # Handle 1-arg and 2-arg structured macros before symbol replacement
    s = replace_cmd_1arg(
        s,
        r"\substack",
        lambda a: clean_latex_expr(a.replace(r"\\", ", ")),
    )
    s = replace_cmd_1arg(s, r"\mathbin", lambda a: f" {clean_latex_expr(a)} ")
    s = replace_cmd_1arg(s, r"\pmod", lambda a: f" (mod {clean_latex_expr(a)})")
    s = replace_cmd_1arg(s, r"\texttt", lambda a: f'<code>{a.replace("&#95;", "_")}</code>')
    s = replace_cmd_1arg(s, r"\text", lambda a: f'<span class="math-text">{a}</span>')
    s = replace_cmd_1arg(s, r"\operatorname", lambda a: f'<span class="math-op">{a}</span>')
    s = replace_cmd_1arg(s, r"\mathbf", lambda a: f"<strong>{clean_latex_expr(a)}</strong>")
    s = replace_cmd_1arg(s, r"\vec", lambda a: f"{clean_latex_expr(a)}&#8407;")
    s = replace_cmd_1arg(s, r"\sqrt", lambda a: f"√({clean_latex_expr(a)})")
    s = replace_cmd_2arg(
        s,
        r"\binom",
        lambda a, b: f"C({clean_latex_expr(a)}, {clean_latex_expr(b)})",
    )
    s = replace_cmd_2arg(
        s,
        r"\frac",
        lambda a, b: f'<span class="math-frac"><span class="num">{clean_latex_expr(a)}</span><span class="slash">/</span><span class="den">{clean_latex_expr(b)}</span></span>',
    )
    s = replace_cmd_2arg(
        s,
        r"\tfrac",
        lambda a, b: f"({clean_latex_expr(a)}/{clean_latex_expr(b)})",
    )

    # Replace LaTeX symbols (longest first) BEFORE stripping sizing macros like \left or \big
    for tex, uni in LATEX_SYMBOLS:
        s = s.replace(tex, uni)

    # Remove sizing and spacing macros safely using word boundaries where applicable
    s = re.sub(r"\\(?:left|right|Big|big|bigg|Bigg)\b", " ", s)
    for sp in [r"\!", r"\,", r"\;", r"\quad", r"\qquad"]:
        s = s.replace(sp, " ")

    # Subscripts and superscripts with braces: _{...} and ^{...}
    s = replace_cmd_1arg(s, "_", lambda a: f"<sub>{clean_latex_expr(a)}</sub>")
    s = replace_cmd_1arg(s, "^", lambda a: f"<sup>{clean_latex_expr(a)}</sup>")

    # Single-character subscripts and superscripts: _x and ^k
    s = re.sub(r"_([A-Za-z0-9+\-∞*])", r"<sub>\1</sub>", s)
    s = re.sub(r"\^([A-Za-z0-9+\-∞*])", r"<sup>\1</sup>", s)

    return s


def preprocess_markdown_lists_and_fences(md_text: str) -> str:
    # 1. Un-indent indented fenced code blocks inside list items so Python-Markdown renders <pre><code>
    def unindent_fence(m):
        lang = m.group(1)
        body = textwrap.dedent(m.group(2))
        return f"\n\n```{lang}\n{body}\n```\n\n"

    md_text = re.sub(
        r"\n[ \t]+```([a-zA-Z0-9_-]*)\n([\s\S]*?)\n[ \t]+```",
        unindent_fence,
        md_text,
    )

    # 2. Normalize 2- or 3-space indented sub-bullets (`  - ` or `   - `) to 4 spaces (`    - `)
    lines = md_text.split("\n")
    out_lines: list[str] = []
    for line in lines:
        m_sub = re.match(r"^( {2,3})(- |\d+\. )(.*)$", line)
        if m_sub:
            if out_lines and out_lines[-1].strip() != "" and not out_lines[-1].startswith("    "):
                out_lines.append("")
            out_lines.append("    " + m_sub.group(2) + m_sub.group(3))
        else:
            # Ensure a blank line before top-level lists when preceded by a prose line OR an indented sub-bullet
            m_top = re.match(r"^(- |\d+\. )(.*)$", line)
            if (
                m_top
                and out_lines
                and out_lines[-1].strip() != ""
                and (
                    out_lines[-1].startswith("    ")
                    or not re.match(r"^(\s*(- |\d+\. )|#|>|\|)", out_lines[-1])
                )
            ):
                out_lines.append("")
            out_lines.append(line)
    return "\n".join(out_lines)


def convert_markdown_with_math_to_html(md_text: str) -> str:
    # Strip SPDX HTML comment header
    md_text = re.sub(r"^<!--.*?-->\s*", "", md_text, flags=re.DOTALL)

    # Preprocess indented code fences and list spacing
    md_text = preprocess_markdown_lists_and_fences(md_text)

    # 1. Stash fenced code blocks and inline code spans using underscore-free tokens
    stash: list[str] = []

    def save_stash(val: str) -> str:
        idx = len(stash)
        stash.append(val)
        return f"@@ICXSTASH{idx:04d}@@"

    md_text = re.sub(
        r"```[\s\S]*?```",
        lambda m: save_stash(m.group(0)),
        md_text,
    )
    md_text = re.sub(
        r"`[^`\n]+`",
        lambda m: save_stash(m.group(0)),
        md_text,
    )

    # 2. Convert display math $$ ... $$
    def repl_display_math(m):
        expr = m.group(1)
        rendered = clean_latex_expr(expr)
        return save_stash(f'\n<div class="math-block">{rendered}</div>\n')

    md_text = re.sub(r"\$\$([\s\S]*?)\$\$", repl_display_math, md_text)

    # 3. Convert inline math $ ... $
    def repl_inline_math(m):
        expr = m.group(1)
        rendered = clean_latex_expr(expr)
        return save_stash(f'<span class="math-inline">{rendered}</span>')

    md_text = re.sub(r"\$([^\$\n]+?)\$", repl_inline_math, md_text)

    # 4. Restore code blocks before running Python-Markdown
    for i, val in enumerate(stash):
        if val.startswith("`"):
            md_text = md_text.replace(f"@@ICXSTASH{i:04d}@@", val)

    html = markdown.markdown(
        md_text,
        extensions=["tables", "fenced_code", "sane_lists", "toc"],
    )

    # 5. Restore all remaining stashes iteratively (handles any nested stashes)
    for _ in range(3):
        if "@@ICXSTASH" not in html:
            break
        for i, val in enumerate(stash):
            tok = f"@@ICXSTASH{i:04d}@@"
            if tok in html:
                if val.startswith("`") and not val.startswith("```"):
                    html = html.replace(tok, f"<code>{val[1:-1]}</code>")
                else:
                    html = html.replace(tok, val)

    # Unescape `\|` inside `<code>...</code>` that were escaped for Markdown table cells
    html = re.sub(
        r"<code>(.*?)</code>",
        lambda m: "<code>" + m.group(1).replace(r"\|", "|") + "</code>",
        html,
    )

    # 6. Wrap standalone <p><img ...></p> + optional caption <p><em>Figure ...</em></p> in <figure class="spec-figure">
    html = re.sub(
        r"<p>\s*(<img[^>]+>)\s*</p>\s*<p>\s*<em>(Figure[^<]+|(?:Vector|Archival).*?)</em>\s*</p>",
        r'<figure class="spec-figure">\1<figcaption>\2</figcaption></figure>',
        html,
        flags=re.DOTALL,
    )
    html = re.sub(
        r"<p>\s*(<img[^>]+>)\s*</p>",
        r'<figure class="spec-figure">\1</figure>',
        html,
    )

    # 7. Style Theorem / Proof paragraphs as callout boxes
    html = re.sub(
        r"<p>\s*<strong>(Theorem[^<]*|Proof:?)</strong>([\s\S]*?)</p>",
        r'<div class="callout-theorem"><strong>\1</strong>\2</div>',
        html,
    )

    # 8. Mark long <pre> blocks (> 30 lines) with class="long-code" so short <pre> blocks avoid page breaks
    html = re.sub(
        r"<pre><code([^>]*)>([\s\S]*?)</code></pre>",
        lambda m: (
            f'<pre class="long-code"><code{m.group(1)}>{m.group(2)}</code></pre>'
            if m.group(2).count("\n") > 30
            else m.group(0)
        ),
        html,
    )

    # 9. Add a dedicated class to the 256-entry lookup table in Chapter 02 so its rows stay single-line and compact
    html = re.sub(
        r"<table>(\s*<thead>\s*<tr>\s*<th[^>]*>Byte \(Hex\)</th>)",
        r'<table class="braille-lookup-table">\1',
        html,
    )

    return html


CSS_STYLES = """
@page {
    size: A4 portrait;
    margin: 16mm 13mm 16mm 13mm;
    @top-left {
        content: "ICX-SPEC-AHTL-1.0 • ALIEN HOLOGRAPHIC TOKEN LATTICE (v1.0)";
        font-family: "DejaVu Sans", sans-serif;
        font-size: 7.5pt;
        font-weight: bold;
        color: #475569;
        letter-spacing: 0.4px;
    }
    @top-right {
        content: string(chapter-id);
        font-family: "DejaVu Sans Mono", monospace;
        font-size: 7.5pt;
        color: #0284C7;
    }
    @bottom-left {
        content: "Infinity-Context (icx) Virtual Context MMU • Normative Specification";
        font-family: "DejaVu Sans", sans-serif;
        font-size: 7.5pt;
        color: #64748B;
    }
    @bottom-right {
        content: "Page " counter(page) " of " counter(pages);
        font-family: "DejaVu Sans Mono", monospace;
        font-size: 7.5pt;
        font-weight: bold;
        color: #0F172A;
    }
}

@page :first {
    margin: 0mm;
    background: #0B1021;
    @top-left { content: none; }
    @top-right { content: none; }
    @bottom-left { content: none; }
    @bottom-right { content: none; }
}

* {
    box-sizing: border-box;
}

body {
    font-family: "DejaVu Sans", "Noto Sans Symbols 2", sans-serif;
    font-size: 9pt;
    line-height: 1.44;
    color: #0F172A;
    margin: 0;
    padding: 0;
}

/* Cover Page */
.cover-page {
    width: 210mm;
    height: 297mm;
    background: #0B1021;
    color: #F8FAFC;
    padding: 18mm 16mm 14mm 16mm;
    page-break-after: always;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
}

.cover-page code {
    background: #1E293B;
    color: #38BDF8;
    border: 1px solid #334155;
    font-size: 7.8pt;
    padding: 1px 4px;
}

.cover-badge-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    border-bottom: 2px solid #00E5FF;
    padding-bottom: 8px;
    margin-bottom: 12px;
}

.cover-badge {
    background: #082F49;
    border: 1px solid #00E5FF;
    color: #00E5FF;
    font-family: "DejaVu Sans Mono", monospace;
    font-size: 8.8pt;
    font-weight: bold;
    padding: 4px 10px;
    border-radius: 4px;
}

.cover-meta-right {
    font-family: "DejaVu Sans Mono", monospace;
    font-size: 8.2pt;
    color: #94A3B8;
}

.cover-title {
    font-size: 22.5pt;
    font-weight: bold;
    color: #F8FAFC;
    line-height: 1.16;
    margin: 4px 0 8px 0;
    letter-spacing: -0.3px;
}

.cover-subtitle {
    font-size: 10.8pt;
    color: #38BDF8;
    line-height: 1.38;
    margin: 0 0 12px 0;
    font-weight: normal;
}

.cover-hero-box {
    background: #050811;
    border: 1.5px solid #1E293B;
    border-radius: 10px;
    padding: 8px;
    margin: 4px 0 10px 0;
    text-align: center;
}

.cover-hero-box img {
    max-width: 100%;
    max-height: 76mm;
    border-radius: 6px;
    display: block;
    margin: 0 auto;
}

.cover-wire-banner {
    background: #091326;
    border: 1px solid #00E5FF;
    border-radius: 6px;
    padding: 7px 10px;
    font-family: "DejaVu Sans", "DejaVu Sans Mono", monospace;
    font-size: 8.2pt;
    color: #6EE7B7;
    text-align: center;
    margin-bottom: 10px;
}

.cover-grid {
    width: 100%;
    border-collapse: separate;
    border-spacing: 8px;
    margin: 2px 0 10px 0;
}

.cover-grid tr, .cover-grid td {
    background: #111827 !important;
}

.cover-grid td {
    width: 50%;
    border: 1px solid #334155;
    border-radius: 8px;
    padding: 8px 10px;
    vertical-align: top;
    color: #E2E8F0 !important;
}

.cover-card-title {
    font-size: 8.8pt;
    font-weight: bold;
    margin-bottom: 4px;
    font-family: "DejaVu Sans", sans-serif;
}

.cover-card-body {
    font-size: 7.8pt;
    color: #CBD5E1 !important;
    line-height: 1.36;
}

.cover-footer {
    border-top: 1px solid #334155;
    padding-top: 8px;
    font-size: 7.8pt;
    color: #94A3B8;
    display: flex;
    justify-content: space-between;
}

/* Table of Contents Page */
.toc-page {
    page-break-after: always;
}

.toc-header-banner {
    background: #0B1021;
    color: #F8FAFC;
    padding: 8px 14px;
    border-radius: 8px;
    border-left: 6px solid #00E5FF;
    margin-bottom: 7px;
}

.toc-header-banner h1 {
    margin: 0;
    font-size: 14pt;
    color: #F8FAFC;
    border: none;
    padding: 0;
}

.toc-header-banner p {
    margin: 2px 0 0 0;
    font-size: 8.2pt;
    color: #94A3B8;
}

.toc-table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 4px;
}

.toc-table th {
    background: #0F172A;
    color: #00E5FF;
    font-size: 7.8pt;
    text-align: left;
    padding: 3.5px 6px;
    border: 1px solid #CBD5E1;
}

.toc-table td {
    padding: 3px 6px;
    border: 1px solid #E2E8F0;
    font-size: 7.5pt;
    vertical-align: top;
    line-height: 1.26;
}

.toc-table tr:nth-child(even) td {
    background: #F8FAFC;
}

/* Chapter Section & Banner */
.chapter-section {
    page-break-before: always;
}

.chapter-banner {
    background: linear-gradient(135deg, #0B1021 0%, #1E293B 100%);
    color: #F8FAFC;
    padding: 10px 14px;
    border-radius: 8px;
    margin-bottom: 11px;
    page-break-inside: avoid;
    break-inside: avoid;
}

.chapter-badge {
    font-family: "DejaVu Sans Mono", monospace;
    font-size: 7.8pt;
    font-weight: bold;
    letter-spacing: 0.6px;
    text-transform: uppercase;
    string-set: chapter-id content();
}

.chapter-banner-title {
    font-size: 14pt;
    font-weight: bold;
    color: #F8FAFC;
    margin: 3px 0 0 0;
}

/* Typography inside chapters */
.chapter-section h1 {
    display: none; /* Replaced by .chapter-banner */
}

h2 {
    font-size: 11.5pt;
    font-weight: bold;
    color: #0B1021;
    border-bottom: 2px solid #0284C7;
    padding-bottom: 2px;
    margin-top: 11px;
    margin-bottom: 6px;
    page-break-after: avoid;
    break-after: avoid;
}

h3 {
    font-size: 10pt;
    font-weight: bold;
    color: #1E293B;
    margin-top: 9px;
    margin-bottom: 4px;
    page-break-after: avoid;
    break-after: avoid;
}

h4 {
    font-size: 9.2pt;
    font-weight: bold;
    color: #334155;
    margin-top: 7px;
    margin-bottom: 3px;
    page-break-after: avoid;
    break-after: avoid;
}

p {
    margin: 4px 0;
    text-align: left;
}

ul, ol {
    margin: 4px 0 6px 0;
    padding-left: 18px;
}

li {
    margin: 2px 0;
}

hr {
    border: none;
    border-top: 1px solid #CBD5E1;
    margin: 8px 0;
}

a {
    color: #0284C7;
    text-decoration: none;
}

/* Inline & Block Code */
code {
    font-family: "DejaVu Sans Mono", "DejaVu Sans", "Noto Sans Symbols 2", monospace;
    font-size: 8pt;
    background: #F1F5F9;
    color: #0F172A;
    padding: 1px 4px;
    border-radius: 3px;
    border: 1px solid #E2E8F0;
    overflow-wrap: break-word;
    word-break: normal;
}

pre {
    background: #0B1021;
    color: #F8FAFC;
    padding: 6.5px 9.5px;
    border-radius: 6px;
    border-left: 4px solid #00E5FF;
    overflow-x: hidden;
    white-space: pre-wrap;
    word-wrap: break-word;
    overflow-wrap: anywhere;
    margin: 4.5px 0;
    page-break-inside: avoid;
    break-inside: avoid;
}

pre.long-code {
    page-break-inside: auto;
    break-inside: auto;
}

pre code {
    background: transparent;
    color: #E2E8F0;
    border: none;
    padding: 0;
    font-size: 7.5pt;
    line-height: 1.34;
    word-break: normal;
}

/* Mathematical Expressions */
.math-block {
    background: #F8FAFC;
    border: 1px solid #CBD5E1;
    border-left: 4px solid #8B5CF6;
    border-radius: 6px;
    padding: 5px 10px;
    margin: 5px 0;
    font-family: "DejaVu Sans", "Noto Sans Symbols 2", sans-serif;
    font-size: 8.8pt;
    color: #0F172A;
    text-align: center;
    page-break-inside: avoid;
    break-inside: avoid;
}

.math-aligned-row {
    margin: 2px 0;
}

.math-inline {
    font-family: "DejaVu Sans", "Noto Sans Symbols 2", sans-serif;
    color: #1E1B4B;
    font-weight: 500;
}

.math-op {
    font-family: "DejaVu Sans", sans-serif;
    font-weight: 600;
    color: #0369A1;
}

.math-text {
    font-family: "DejaVu Sans", sans-serif;
}

.math-frac {
    display: inline-block;
    padding: 0 2px;
}

.matrix-wrap, .cases-wrap {
    display: inline-block;
    vertical-align: middle;
    margin: 2px 4px;
}

table.math-matrix {
    display: inline-table;
    border-collapse: collapse;
    border-left: 1.8px solid #0F172A;
    border-right: 1.8px solid #0F172A;
    border-radius: 6px;
    padding: 2px 6px;
    vertical-align: middle;
}

table.math-cases {
    display: inline-table;
    border-collapse: collapse;
    border-left: 2px solid #0F172A;
    border-radius: 4px;
    padding: 2px 6px;
    vertical-align: middle;
}

table.math-matrix td, table.math-cases td {
    border: none !important;
    background: transparent !important;
    padding: 1px 6px !important;
    font-size: 8pt;
    text-align: center;
}

/* Theorem / Proof Callout Boxes */
.callout-theorem {
    background: #F5F3FF;
    border: 1px solid #DDD6FE;
    border-left: 4px solid #7C3AED;
    border-radius: 6px;
    padding: 6px 10px;
    margin: 6px 0;
    page-break-inside: avoid;
    break-inside: avoid;
}

/* Figures & Diagrams */
figure.spec-figure {
    margin: 7px 0;
    padding: 6px;
    background: #F8FAFC;
    border: 1px solid #CBD5E1;
    border-radius: 7px;
    text-align: center;
    page-break-inside: avoid;
    break-inside: avoid;
}

figure.spec-figure img {
    max-width: 100%;
    max-height: 76mm;
    width: auto;
    height: auto;
    display: block;
    margin: 0 auto;
    border-radius: 4px;
}

figure.spec-figure figcaption {
    margin-top: 4px;
    font-size: 7.8pt;
    color: #334155;
    font-style: italic;
    line-height: 1.32;
}

/* Tables */
table {
    width: 100%;
    border-collapse: collapse;
    margin: 6px 0;
    font-size: 7.5pt;
}

thead {
    display: table-header-group;
}

tr {
    page-break-inside: avoid;
    break-inside: avoid;
}

th {
    background: #0F172A;
    color: #F8FAFC;
    font-weight: bold;
    padding: 4px 5.5px;
    border: 1px solid #334155;
    text-align: left;
}

th code {
    background: #1E293B;
    color: #38BDF8;
    border: 1px solid #334155;
    font-size: 7.2pt;
    white-space: normal;
    word-break: break-all;
}

th .math-inline, th .math-op, th .math-text, th sub, th sup {
    color: #38BDF8 !important;
}

td {
    padding: 3.5px 5.5px;
    border: 1px solid #CBD5E1;
    vertical-align: middle;
    word-wrap: break-word;
    overflow-wrap: break-word;
}

td code {
    overflow-wrap: anywhere;
    word-break: break-word;
}

tr:nth-child(even) td {
    background: #F8FAFC;
}

td img {
    max-width: 100%;
    max-height: 34mm;
    width: auto;
    height: auto;
    display: block;
    margin: 2px auto;
}

/* Compact 256-Entry Braille Lookup Table */
table.braille-lookup-table {
    font-size: 6.8pt;
}

table.braille-lookup-table th,
table.braille-lookup-table td {
    padding: 1.3px 4px;
    line-height: 1.2;
}

#ch-01 figure.spec-figure img {
    max-height: 64mm;
}

#ch-03 figure.spec-figure img {
    max-height: 60mm;
}

#ch-03 td img {
    max-height: 27mm;
}

table.braille-lookup-table td:nth-child(1),
table.braille-lookup-table td:nth-child(2),
table.braille-lookup-table td:nth-child(3),
table.braille-lookup-table td:nth-child(4),
table.braille-lookup-table td:nth-child(5),
table.braille-lookup-table td:nth-child(6),
table.braille-lookup-table td:nth-child(1) code,
table.braille-lookup-table td:nth-child(2) code,
table.braille-lookup-table td:nth-child(4) code,
table.braille-lookup-table td:nth-child(5) code,
table.braille-lookup-table td:nth-child(6) code {
    white-space: nowrap !important;
    word-break: normal !important;
    overflow-wrap: normal !important;
    text-align: center;
}

table.braille-lookup-table td:nth-child(5) code {
    font-size: 9pt;
    padding: 0 3px;
}
"""


def build_cover_and_toc_html() -> str:
    raw = """
  <!-- COVER PAGE -->
  <div class="cover-page">
    <div>
      <div class="cover-badge-row">
        <span class="cover-badge">DOCUMENT ID: ICX-SPEC-AHTL-1.0</span>
        <span class="cover-meta-right">INFINITY-CONTEXT (`icx`) • VIRTUAL CONTEXT MMU • v1.1.0</span>
      </div>
      <div class="cover-title">
        Alien Holographic Token Lattice (`AHTL v1.0`)<br/>
        Complete Technical Specification Volume
      </div>
      <div class="cover-subtitle">
        Normative Mathematical, Algorithmic, Cryptographic &amp; Systems Architecture Specification
        for Non-Human BPE-Aligned Context Compression, Dual-Hypercube Associative Indexing, and Lossless Merkle-DAG Paging
      </div>

      <div class="cover-hero-box">
        <img src="images/illustrations/AHTL_Holographic_Lattice_Concept.png" alt="AHTL v1.0 Holographic Lattice Concept"/>
      </div>

      <div class="cover-wire-banner">
        ⟦AHTL v1│#icx:4e81a0c2f91b│fc:4000c000...│hv:123456789abcdef0│⠞⠧⡓⣠⠵⠷⢾⣿... (64-Braille)│⊕fn:alloc_page⋮!PANIC@L99⟧
      </div>

      <table class="cover-grid">
        <tr>
          <td style="border-top: 3px solid #00E5FF;">
            <div class="cover-card-title" style="color: #00E5FF;">1. BLAKE3 Merkle-DAG CAS (`#icx:&lt;hex&gt;`)</div>
            <div class="cover-card-body">
              SplitMix64 gear-hash FastCDC (`2..64 KiB`) + 1,024-byte Left-Full binary Merkle tree (`Bao`).
              48-bit (`#icx:12-hex`) &amp; 64-bit (`#icx:16-hex`) virtual handles verified against full 256-bit `Blake3Digest` in `&lt; 50 µs`.
            </div>
          </td>
          <td style="border-top: 3px solid #10B981;">
            <div class="cover-card-title" style="color: #34D399;">2. Contractive IFS Fractal Coordinates (`fc:&lt;64-hex&gt;`)</div>
            <div class="cover-card-body">
              16-dimensional origin-centered continuous hypercube `[-1, +1]^16` (`[i16; 16]` Q15).
              Banach contraction (`L = 0.5 &lt; 1`) yields exact `O(1)` Chebyshev `L_∞` trailing state-suffix depth in `&lt; 10 ns`.
            </div>
          </td>
        </tr>
        <tr>
          <td style="border-top: 3px solid #8B5CF6;">
            <div class="cover-card-title" style="color: #C084FC;">3. 10,240-Bit Hypercube Lattice (`hv:&lt;16-hex&gt;`)</div>
            <div class="cover-card-body">
              Discrete Boolean hypercube `Q_10240` (`[u64; 160]`; portable safe-Rust word loops, no explicit SIMD intrinsics).
              BLAKE3 XOF basis, XOR role-filler binding, cyclic permutation, Infini-attention Delta bundler, and `12 ns` `VPOPCNTDQ` scan.
            </div>
          </td>
          <td style="border-top: 3px solid #F59E0B;">
            <div class="cover-card-title" style="color: #FBBF24;">4. ISO 11548-1 Braille-256 &amp; AST Skeleton</div>
            <div class="cover-card-body">
              Bijective 8-dot Unicode Braille mapping (`U+2800 + b`, 256 states) in orthogonal `So` subspace +
              single-pass `O(N)` AST skeletonizer &amp; `SecretRedactor` Tiers 0-3 (lossless storage after redaction; 95.46% byte-weighted prefix reuse on the synthetic benchmark transcript).
            </div>
          </td>
        </tr>
      </table>
    </div>

    <div class="cover-footer">
      <span><strong>Workspace Crates:</strong> `icx-core` · `icx-hdc` · `icx-codec` · `icx-store` · `icx-bus` · `icx-adapters` · `icx-proxy` · `icx-cli`</span>
      <span><strong>License:</strong> CC-BY-NC-ND-4.0 / GPL-3.0-or-later</span>
    </div>
  </div>

  <!-- TABLE OF CONTENTS PAGE -->
  <div class="toc-page">
    <div class="toc-header-banner">
      <h1>MASTER TABLE OF CONTENTS &amp; SPECIFICATION STRUCTURE</h1>
      <p>Complete 9-Part Technical Specification Volume Synthesizing All 4 Foundational Reference Archives &amp; Workspace Crates</p>
    </div>

    <table class="toc-table">
      <thead>
        <tr>
          <th style="width: 12%;">Section ID</th>
          <th style="width: 34%;">Specification Chapter Title</th>
          <th style="width: 38%;">Key Normative Topics, Proofs &amp; Tables</th>
          <th style="width: 16%;">Target Crates</th>
        </tr>
      </thead>
      <tbody>
        <tr>
          <td><strong>EXEC-00</strong></td>
          <td><strong>Executive Overview &amp; Master Architecture</strong><br/><code>docs/ahtl/README.md</code></td>
          <td>5-stage holographic pipeline, master index, visual asset taxonomy (`generated/`, `illustrations/`, `retrieved/`), and end-to-end latency budgets (`197.85 µs`).</td>
          <td>All 8 Crates</td>
        </tr>
        <tr>
          <td><strong>CH-01</strong></td>
          <td><strong>Formal Grammar, Wire Profiles &amp; Packet Spec</strong><br/><code>01-formal-grammar-and-packet-specification.md</code></td>
          <td>ISO 14977 EBNF grammar for Profile A (`5-Field`) &amp; Profile B (`Inline`), delimiter codepoint table (`U+27E6..U+21A6`), cross-tokenizer budgets, LL(1) reverse-split FSM, test vectors.</td>
          <td><code>icx-codec</code><br/><code>icx-core</code></td>
        </tr>
        <tr>
          <td><strong>CH-02</strong></td>
          <td><strong>Braille-256 Spectral Bijection (`U+2800..U+28FF`)</strong><br/><code>02-braille-256-spectral-bijection.md</code></td>
          <td>ISO/TR 11548-1 8-dot numbering (`1-2-3-7 | 4-5-6-8`), hex dot weights (`0x01..0x80`), closed-form 3-byte UTF-8 arithmetic, 64-band &amp; 8-byte spectral folding, <strong>full 256-entry lookup table</strong>.</td>
          <td><code>icx-codec</code><br/><code>icx-hdc</code></td>
        </tr>
        <tr>
          <td><strong>CH-03</strong></td>
          <td><strong>Contractive IFS &amp; Fractal Chaos-Game (`fc:`)</strong><br/><code>03-fractal-ifs-chaos-game-coordinates.md</code></td>
          <td>`[-1, +1]^16` complete Chebyshev space, block-majority projection, Banach Fixed-Point Theorem proof (`L = 0.5 &lt; 1`), <strong>`O(1)` Chebyshev Suffix Theorem proof</strong>, `Q15` / `Q1.14` serialization.</td>
          <td><code>icx-hdc</code><br/><code>icx-bus</code></td>
        </tr>
        <tr>
          <td><strong>CH-04</strong></td>
          <td><strong>10,240-Bit Hyperdimensional Lattice (`hv:`)</strong><br/><code>04-hyperdimensional-10240-bit-lattice.md</code></td>
          <td>Boolean hypercube `Q_10240` (`[u64; 160]`), BLAKE3 XOF &amp; SplitMix64 basis, concentration of measure (`μ=5120, σ=50.60`), Coxeter group `B_10240`, Infini-attention Delta bundler.</td>
          <td><code>icx-hdc</code></td>
        </tr>
        <tr>
          <td><strong>CH-05</strong></td>
          <td><strong>Structural AST Skeletonization &amp; Redaction</strong><br/><code>05-structural-ast-skeletonization-and-redaction.md</code></td>
          <td>Single-pass `O(N)` homomorphic AST pruning (Rust, Python, TS, Go, logs, diffs), unmangled identifier invariant, Bi-Temporal CSR Graph edges, and the Tier 0-3 `SecretRedactor` (`H(S) &gt; 4.35`).</td>
          <td><code>icx-codec</code><br/><code>icx-store</code></td>
        </tr>
        <tr>
          <td><strong>CH-06</strong></td>
          <td><strong>BLAKE3 Merkle-DAG CAS &amp; Virtual Handles</strong><br/><code>06-blake3-merkle-cas-and-virtual-handles.md</code></td>
          <td>SplitMix64 gear-hash `FastCDC`, BLAKE3 7-round ChaCha ARX &amp; 1,024B Left-Full Bao tree, domain flags `d`, 4-tier `MerkleContextDag`, birthday collision bounds, Single-Tool MCP Pager (`icx`).</td>
          <td><code>icx-core</code><br/><code>icx-adapters</code></td>
        </tr>
        <tr>
          <td><strong>CH-07</strong></td>
          <td><strong>Transformer Attention, BPE &amp; Cache Epochs</strong><br/><code>07-transformer-attention-and-bpe-mechanics.md</code></td>
          <td>BPE pre-tokenization barriers, Softmax partition noise elimination (`18.4×–72.1×` SNR boost), `⟦...⟧` attention sinks, 2-layer induction-head pointer copy, Titans-inspired surprise `S_t`, prefix-reuse proxy (not a measured provider hit rate).</td>
          <td><code>icx-core</code><br/><code>icx-proxy</code></td>
        </tr>
        <tr>
          <td><strong>CH-08</strong></td>
          <td><strong>Reference Implementation Blueprint</strong><br/><code>08-v1-reference-implementation-blueprint.md</code></td>
          <td>Complete Rust implementation for `AhtlWirePacketV1` (Profile A) and `AhtlPacket` (Profile B), 64-band spectral projection, SIMD intrinsics, and the 10-invariant verification test matrix.</td>
          <td>All 8 Crates</td>
        </tr>
      </tbody>
    </table>

    <figure class="spec-figure" style="margin-top: 6px; padding: 4px;">
      <img src="images/generated/ahtl-packet-anatomy.png" alt="AHTL Packet Anatomy Overview" style="max-height: 31mm;"/>
      <figcaption>Master Reference Overview: Anatomy of an Alien Holographic Token Lattice (`AHTL v1.0`) packet.</figcaption>
    </figure>
  </div>
"""
    return re.sub(r"`([^`\n]+)`", r"<code>\1</code>", raw)


def main() -> None:
    sections_html = [build_cover_and_toc_html()]

    for sec_id, sec_title, filename, accent_color in CHAPTER_FILES:
        md_path = AHTL_DIR / filename
        raw_md = md_path.read_text(encoding="utf-8")
        body_html = convert_markdown_with_math_to_html(raw_md)
        sec_block = f"""
  <section class="chapter-section" id="{sec_id.lower()}">
    <div class="chapter-banner" style="border-left: 6px solid {accent_color};">
      <div class="chapter-badge" style="color: {accent_color};">ICX-SPEC-AHTL-1.0 • {sec_id} • {filename}</div>
      <div class="chapter-banner-title">{sec_title}</div>
    </div>
    {body_html}
  </section>
"""
        sections_html.append(sec_block)

    full_html = f"""<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8"/>
  <title>AHTL v1.0 Complete Technical Specification Volume — Infinity-Context (icx)</title>
  <style>{CSS_STYLES}</style>
</head>
<body>
{"".join(sections_html)}
</body>
</html>
"""
    doc = weasyprint.HTML(string=full_html, base_url=str(AHTL_DIR) + "/")
    doc.write_pdf(str(OUT_PDF))
    print(f"Successfully generated {OUT_PDF} ({OUT_PDF.stat().st_size:,} bytes)")


if __name__ == "__main__":
    main()
