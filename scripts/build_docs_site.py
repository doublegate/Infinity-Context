#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 DoubleGate
# SPDX-License-Identifier: GPL-3.0-or-later
"""Stage the documentation site for Zensical.

The Markdown in `docs/` and the root pages (README, CHANGELOG, ...) are written for browsing on
GitHub: they link to source files, `ref-docs/` and root images by relative path. A static site
only contains `docs_dir`, so those links would break. This script copies everything into a
staging directory and rewrites each link:

- a page that is part of the site keeps a relative link to its staged copy;
- an image outside `docs/` is copied in under `_repo/` and linked there;
- anything else (source files, `ref-docs/`, `to-dos/`) links to the file on GitHub.

It also writes the site configuration with a navigation built from the curated order below plus
every other page found, so a new document appears on the site without editing the config.
The sources are never modified. Standard library only.

Usage: python3 scripts/build_docs_site.py [--out .site-src] [--config zensical.toml]
Then:  zensical build --clean --strict -f .site-src/zensical.toml   (output: .site-src/site)
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import io
import posixpath
import re
import shutil
import sys
import tarfile
import time
import urllib.request
from pathlib import Path

REPO = Path(__file__).resolve().parent.parent
GITHUB = "https://github.com/doublegate/Infinity-Context"
BRANCH = "main"

# Root pages published on the site: repository path -> staged path.
ROOT_PAGES = {
    "README.md": "index.md",
    "CHANGELOG.md": "changelog.md",
    "PROJECT-STATUS.md": "project-status.md",
    "CONTRIBUTING.md": "contributing.md",
    "SECURITY.md": "security-policy.md",
    "LICENSING.md": "licensing.md",
}

# Files outside docs/ the theme itself needs (logo, favicon), copied in under _repo/.
STATIC = ["assets/icons/png/icon-128.png", "assets/web/favicon-32x32.png"]

# MathJax is self-hosted, not loaded from a CDN: the npm package is downloaded at staging time
# and must match npm's published integrity hash, so a compromised CDN or registry cannot change
# the script the site serves. Only the files the site needs are extracted.
MATHJAX_URL = "https://registry.npmjs.org/mathjax/-/mathjax-3.2.2.tgz"
MATHJAX_SHA512 = "Bt+SSVU8eBG27zChVewOicYs7Xsdt40qm4+UpHyX7k0/O9NliPc+x77k1/FEsPsjKPZGJvtRZM1vO+geW0OhGw=="
MATHJAX_FILES = re.compile(
    r"^package/(LICENSE|es5/tex-mml-chtml\.js|es5/output/chtml/fonts/woff-v2/[A-Za-z0-9_-]+\.woff)$"
)

IMAGE_EXT = {".png", ".svg", ".jpg", ".jpeg", ".gif", ".webp", ".ico"}

# Curated navigation; pages not listed are appended to the section matching their folder, or
# to "More" (so nothing is ever left out of the navigation).
NAV = [
    ("Home", ["index.md"]),
    (
        "Getting Started",
        [
            "deployment-and-agent-integration-guide.md",
            "build-and-tooling.md",
            "glossary.md",
        ],
    ),
    (
        "Architecture",
        [
            "architecture.md",
            "alien-holographic-codec.md",
            "virtual-memory-and-dag-engine.md",
            "hyperdimensional-and-fractal-index.md",
            "agent-adapters-and-interception.md",
            "capability-matrix.md",
            "wire-protocol-and-shm-bus.md",
            "data-model.md",
            "interfaces.md",
            "diagrams/",
        ],
    ),
    ("AHTL Specification", ["ahtl/"]),
    (
        "Quality & Security",
        [
            "performance.md",
            "numbers.md",
            "testing-strategy.md",
            "security-and-threat-model.md",
            "security-policy.md",
        ],
    ),
    ("Decisions (ADRs)", ["adr/"]),
    ("Releases", ["changelog.md", "project-status.md", "releases/"]),
    ("Audits", ["claude/", "codex/"]),
    ("Project", ["contributing.md", "licensing.md"]),
]

LINK_MD = re.compile(r"(\]\()([^)\s]+)((?:\s+\"[^\"]*\")?\))")
LINK_HTML = re.compile(r"""((?:src|href)=")([^"]+)(")""")
SRCSET = re.compile(r"""(srcset=")([^"]+)(")""")
CODE_SPAN = re.compile(r"(`+)(?:(?!\1).)+?\1")
FENCE = re.compile(r"^\s*(`{3,}|~{3,})")

# Link targets that do not exist in the repository (reported, and the build fails).
MISSING: list[str] = []


def staged_page(repo_path: str) -> str | None:
    """The staged path of a repository file that is a page or asset of the site."""
    if repo_path.startswith("docs/"):
        # Scripts under docs/ are not copied to the site; they link to GitHub instead.
        return None if repo_path.endswith(".py") else repo_path[len("docs/") :]
    return ROOT_PAGES.get(repo_path)


def rewrite_target(target: str, src_repo: str, src_staged: str, out: Path) -> str:
    """Rewrite one link target found in `src_repo` (staged as `src_staged`)."""
    if re.match(r"^[a-z][a-z0-9+.-]*:", target) or target.startswith(("#", "/")):
        return target
    path, sep, frag = target.partition("#")
    if not path:
        return target
    repo_path = posixpath.normpath(posixpath.join(posixpath.dirname(src_repo), path))
    if repo_path.startswith("../") or repo_path == "..":
        MISSING.append(f"{src_repo}: {target} (outside the repository)")
        return target
    if not (REPO / repo_path).exists():
        MISSING.append(f"{src_repo}: {target}")
        return target
    staged = staged_page(repo_path)
    # A directory links to its index page on the site (#36), or to GitHub when it has none.
    if staged is not None and (REPO / repo_path).is_dir():
        index = next(
            (n for n in ("index.md", "README.md") if (REPO / repo_path / n).is_file()),
            None,
        )
        staged = posixpath.join(staged, index) if index else None
    if (
        staged is None
        and (REPO / repo_path).is_file()
        and Path(repo_path).suffix in IMAGE_EXT
    ):
        staged = f"_repo/{repo_path}"
        dest = out / staged
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / repo_path, dest)
    if staged is None:
        kind = "tree" if (REPO / repo_path).is_dir() else "blob"
        return f"{GITHUB}/{kind}/{BRANCH}/{repo_path}" + (sep + frag if frag else "")
    rel = posixpath.relpath(staged, posixpath.dirname(src_staged) or ".")
    return rel + (sep + frag if frag else "")


def rewrite_text(text: str, src_repo: str, src_staged: str, out: Path) -> str:
    lines, fence = [], ""
    for line in text.splitlines(keepends=True):
        m = FENCE.match(line)
        if m and not fence:
            fence = m.group(1)
        elif (
            m
            and m.group(1)[0] == fence[0]
            and len(m.group(1)) >= len(fence)
            and not line[m.end() :].strip()
        ):
            # A closing fence carries no info string (CommonMark).
            fence = ""
        elif not fence:

            def fix(m: re.Match[str]) -> str:
                return (
                    m.group(1)
                    + rewrite_target(m.group(2), src_repo, src_staged, out)
                    + m.group(3)
                )

            def fix_srcset(m: re.Match[str]) -> str:
                # `a.png 1x, b.png 2x`: rewrite each candidate URL, keep its descriptor.
                candidates = []
                for candidate in m.group(2).split(","):
                    url, _, descriptor = candidate.strip().partition(" ")
                    url = rewrite_target(url, src_repo, src_staged, out)
                    candidates.append(f"{url} {descriptor}".strip())
                return m.group(1) + ", ".join(candidates) + m.group(3)

            def fix_prose(segment: str) -> str:
                return SRCSET.sub(
                    fix_srcset, LINK_HTML.sub(fix, LINK_MD.sub(fix, segment))
                )

            # Links inside inline code spans are examples, not links: leave them as written.
            parts, last = [], 0
            for span in CODE_SPAN.finditer(line):
                parts.append(fix_prose(line[last : span.start()]))
                parts.append(span.group(0))
                last = span.end()
            parts.append(fix_prose(line[last:]))
            line = "".join(parts)
        lines.append(line)
    return "".join(lines)


def vendor_mathjax(out: Path) -> None:
    """Download the pinned MathJax package, verify it, and extract the needed files."""
    # A verified copy is cached in .site-cache/ (gitignored) so local previews work offline.
    cache = REPO / ".site-cache" / MATHJAX_URL.rsplit("/", 1)[1]

    def verified(blob: bytes) -> bool:
        return (
            base64.b64encode(hashlib.sha512(blob).digest()).decode() == MATHJAX_SHA512
        )

    data = cache.read_bytes() if cache.is_file() else b""
    if not verified(data):
        # The docs check runs on most pull requests: retry a transient registry error (#36).
        for attempt in range(4):
            try:
                with urllib.request.urlopen(MATHJAX_URL, timeout=60) as response:
                    data = response.read()
                break
            except OSError as err:
                if attempt == 3:
                    raise SystemExit(f"MathJax download failed: {err}") from err
                time.sleep(2**attempt)
        if not verified(data):
            raise SystemExit("MathJax package integrity mismatch: refusing to use it")
        cache.parent.mkdir(parents=True, exist_ok=True)
        cache.write_bytes(data)
    dest_root = out / "javascripts" / "mathjax"
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as archive:
        for member in archive.getmembers():
            if not member.isfile() or not MATHJAX_FILES.match(member.name):
                continue
            relative = member.name.removeprefix("package/").removeprefix("es5/")
            target = dest_root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            source = archive.extractfile(member)
            if source is None:
                continue
            target.write_bytes(source.read())
    if not (dest_root / "tex-mml-chtml.js").is_file():
        raise SystemExit("MathJax package did not contain es5/tex-mml-chtml.js")


def stage(out: Path) -> list[str]:
    if out.exists():
        shutil.rmtree(out)
    shutil.copytree(
        REPO / "docs", out, ignore=shutil.ignore_patterns("*.py", "__pycache__")
    )
    # Site-only assets (the MathJax configuration) live outside docs/.
    shutil.copytree(REPO / "scripts" / "site", out, dirs_exist_ok=True)
    vendor_mathjax(out)
    for repo_path in STATIC:
        dest = out / "_repo" / repo_path
        dest.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(REPO / repo_path, dest)
    pages = []
    for md in sorted(out.rglob("*.md")):
        staged = md.relative_to(out).as_posix()
        text = md.read_text(encoding="utf-8")
        md.write_text(
            rewrite_text(text, f"docs/{staged}", staged, out), encoding="utf-8"
        )
        pages.append(staged)
    for repo_path, staged in ROOT_PAGES.items():
        text = (REPO / repo_path).read_text(encoding="utf-8")
        (out / staged).write_text(
            rewrite_text(text, repo_path, staged, out), encoding="utf-8"
        )
        pages.append(staged)
    return sorted(set(pages))


def nav_entries(pages: list[str]) -> list[tuple[str, list[str]]]:
    used: set[str] = set()
    nav = []
    for title, items in NAV:
        entries = []
        for item in items:
            if item.endswith("/"):
                folder = sorted(
                    (p for p in pages if p.startswith(item) and p not in used),
                    key=lambda p: (p.count("/"), not p.endswith("README.md"), p),
                )
                entries.extend(folder)
                used.update(folder)
            elif item in pages and item not in used:
                entries.append(item)
                used.add(item)
        if entries:
            nav.append((title, entries))
    rest = [p for p in pages if p not in used]
    if rest:
        nav.append(("More", rest))
    return nav


def toml_nav(nav: list[tuple[str, list[str]]]) -> str:
    def q(s: str) -> str:
        return '"' + s.replace("\\", "\\\\").replace('"', '\\"') + '"'

    sections = []
    for title, entries in nav:
        if title == "Home" and entries == ["index.md"]:
            sections.append('  { "Home" = "index.md" }')
            continue
        sections.append(
            f"  {{ {q(title)} = [\n"
            + ",\n".join(f"    {q(e)}" for e in entries)
            + "\n  ] }"
        )
    return "nav = [\n" + ",\n".join(sections) + "\n]\n"


def main() -> int:
    ap = argparse.ArgumentParser(
        description="Stage the documentation site for Zensical."
    )
    ap.add_argument(
        "--out", default=".site-src", help="staging directory (default .site-src)"
    )
    ap.add_argument(
        "--config", default="zensical.toml", help="committed site configuration"
    )
    args = ap.parse_args()
    out = (REPO / args.out).resolve()
    if out == REPO or REPO not in out.parents:
        print(f"refusing to stage outside the repository: {out}", file=sys.stderr)
        return 2
    docs = out / "docs"
    pages = stage(docs)
    config = (REPO / args.config).read_text(encoding="utf-8")
    # Line-ending agnostic, so a CRLF checkout works too (#36).
    header = re.compile(r"(?m)^\[project\][ \t]*\r?$\n?")
    if not header.search(config):
        print(f"{args.config}: no [project] table", file=sys.stderr)
        return 2
    nav = toml_nav(nav_entries(pages))
    config = header.sub(
        lambda m: m.group(0).rstrip("\r\n") + "\n" + nav, config, count=1
    )
    config = re.sub(r"(?m)^docs_dir\s*=.*$", 'docs_dir = "docs"', config)
    config = re.sub(r"(?m)^site_dir\s*=.*$", 'site_dir = "site"', config)
    (out / "zensical.toml").write_text(config, encoding="utf-8")
    if MISSING:
        print("links to files that do not exist:", file=sys.stderr)
        for item in MISSING:
            print(f"  {item}", file=sys.stderr)
        return 1
    print(f"staged {len(pages)} pages in {out.relative_to(REPO)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
