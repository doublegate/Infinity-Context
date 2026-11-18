<!--
SPDX-FileCopyrightText: 2026 Luke R. Parobek
SPDX-FileCopyrightText: 2026 DoubleGate
SPDX-License-Identifier: CC-BY-NC-ND-4.0 AND LicenseRef-InfinityContext-Commercial
-->

# Deployment & Multi-Agent Integration Guide (`v1.0.0`)

**References:** [docs/architecture.md](architecture.md), [docs/agent-adapters-and-interception.md](agent-adapters-and-interception.md), [docs/wire-protocol-and-shm-bus.md](wire-protocol-and-shm-bus.md), [docs/virtual-memory-and-dag-engine.md](virtual-memory-and-dag-engine.md), [ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md](../ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md), [ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md](../ref-docs/agent-internals/cross-agent-hook-and-wire-matrix.md)  
**Target Release:** `v1.0.0` (End of Phase 5)  
**Target Crates:** `icx-cli`, `icx-adapters`, `icx-proxy`, `icx-bus`, `icx-store`, `icx-codec`, `icx-hdc`, `icx-core`

---

## Table of Contents

1. [Executive Architectural Overview & Memory Tier Matrix](#1-executive-architectural-overview--memory-tier-matrix)
2. [End-to-End Setup & Configuration Guide (Mode A: Standalone `icx`)](#2-end-to-end-setup--configuration-guide-mode-a-standalone-icx)
3. [Dual-Stack Integration (`icx` + `ai-memory` Coexistence Architecture — Mode B)](#3-dual-stack-integration-icx--ai-memory-coexistence-architecture--mode-b)
4. [Co-Existence with Native Agent Context Windows & Compaction Behavior](#4-co-existence-with-native-agent-context-windows--compaction-behavior)
5. [Recommendations & Best Practices for Use](#5-recommendations--best-practices-for-use)
6. [Realized Benefits, Pros/Cons, and Trade-Off Matrix](#6-realized-benefits-proscons-and-trade-off-matrix)
7. [Typical End-to-End Use Cases & Turn-by-Turn Workflows](#7-typical-end-to-end-use-cases--turn-by-turn-workflows)
8. [Verification, Smoke-Testing, Diagnostics & Systemd Service Guide](#8-verification-smoke-testing-diagnostics--systemd-service-guide)

---

## 1. Executive Architectural Overview & Memory Tier Matrix

At `v1.0.0` (completion of Phase 5), **Infinity-Context (`icx`)** operates as a cross-agent Virtual Context Memory Management Unit (MMU) across **Claude Code (`2.1.326+`)**, **OpenAI Codex CLI (`0.169.0+`)**, **Google Antigravity CLI (`agy`)**, **Hermes Agent**, and generic Model Context Protocol (`MCP`) / Agent Client Protocol (`ACP`) clients.

Workstations and CI/CD environments can deploy `icx` in two supported architectures:

- **Mode A — Standalone Infinity-Context (`icx` Only, Without `ai-memory`):** `icx` handles all working, episodic, and workspace-persisted context via its L0–L4 Virtual Context MMU, Alien Holographic Token Lattice (`AHTL`) codec, 10,240-bit Hyperdimensional (`HDC`) + Contractive IFS Fractal (`FCGA`) + Bi-Temporal Personalized PageRank (`PPR`) associative index, `/dev/shm` Seqlock Telepathy Bus, Single-Tool Polymorphic MCP Pager (`icx`), and Cache-Epoch HTTP/1.1 & HTTP/2 Wire Proxy (`127.0.0.1:49380` / `127.0.0.1:8787`).
- **Mode B — Dual-Stack Coexistence (`icx` + `ai-memory` Skills & Daemon):** `icx` operates the sub-500us machine-native L0–L4 Virtual Context MMU, in-flight tool output virtualizer (`>= 512` bytes default / configurable `>= 2,048` bytes), lossless BLAKE3 Merkle-DAG (`LCM`), and sub-500ns `/dev/shm` cross-agent telepathy ring, while `ai-memory` (`127.0.0.1:49375`) manages the human-curated Markdown Wiki (`_rules/`, `decisions/`, `gotchas/`, `procedures/`, `concepts/`, `profile/`), cross-session operator handoffs, and cross-repository agent inboxes via the six `ai-memory-*` skills.

```mermaid
flowchart TB
    subgraph Harnesses["AI Coding & Research Harnesses"]
        CC["Claude Code (2.1.326+)"]
        CX["OpenAI Codex CLI (0.169.0+)"]
        AGY["Google Antigravity CLI (agy)"]
        HM["Hermes Agent (ContextEngine + MemoryProvider)"]
        GEN["Generic MCP / ACP Clients (Cursor, Windsurf, Cline, OpenCode)"]
    end

    subgraph ICX["Plane A: Infinity-Context (icx v1.0.0) — Sub-500us Machine-Native MMU"]
        HOOKS["Native Hook Adapters (crates/icx-adapters)\nPostToolUse Stubbing + Pre-Model Ephemeral Tail Slot"]
        MCP_ICX["Single-Tool Polymorphic MCP Pager (icx)\nderef | slice | grep | recall | fold | expand | xagent | status"]
        PROXY["Cache-Epoch Wire Proxy (crates/icx-proxy)\n127.0.0.1:49380 or :8787 (/anthropic, /openai/v1, /gemini)"]
        SHM["/dev/shm/icx-fabric-v1 (or .icx/icx-fabric-v1.shm)\nLock-Free 64B Header + 256 x 4096B Seqlock Ring (<500ns)"]
        MMU["InfinityContextMmu (.icx/ Workspace State)\nL0 FastCDC/BLAKE3 CAS + L1-L4 LCM Merkle-DAG\n10,240-Bit HDC + 16-D FCGA + CSR Bi-Temporal PPR Graph"]
    end

    subgraph AIM["Plane B: ai-memory (Optional Mode B) — Human-Curated Semantic Wiki"]
        AIM_SKILLS["6 Agent Skills (ai-memory-*)\nretrieval | durable-pages | handoff | learning-maintenance | messaging | routing-install"]
        AIM_DAEMON["ai-memory HTTP/MCP Daemon\n127.0.0.1:49375 + memory.sqlite + wiki/*.md"]
    end

    CC & CX & AGY & HM & GEN --> HOOKS
    CC & CX & AGY & HM & GEN --> MCP_ICX
    CC & CX & AGY & GEN --> PROXY
    HOOKS & MCP_ICX & PROXY --> SHM
    HOOKS & MCP_ICX & PROXY --> MMU
    CC & CX & AGY & HM & GEN -. "Mode B Only" .-> AIM_SKILLS
    AIM_SKILLS -. "Mode B Only" .-> AIM_DAEMON
```

### 1.1 Memory Tier & Responsibility Matrix (`icx` vs. `ai-memory`)

| Dimension | Infinity-Context (`icx` `v1.0.0`) | `ai-memory` Daemon & Skills (Mode B Only) | Standalone Coverage in Mode A (`icx` Without `ai-memory`) |
| :--- | :--- | :--- | :--- |
| **Primary Role** | Machine-native Virtual Context MMU, in-flight tool output virtualizer, lossless byte-exact CAS, and sub-microsecond cross-agent telepathy bus. | Human-readable Markdown Wiki, operator-curated rules/decisions, single-use session handoffs, and cross-project agent mail queue. | `icx` persists full workspace state atomically under `.icx/` and exposes associative + graph recall across sessions. |
| **Wire Representation** | Non-human BPE-aligned **Alien Holographic Token Lattice (`AHTL`)** (`Profile A` 6-section wire & `Profile B` dual-channel clauses + Braille-256 topological sketch). | Human-authored/compiled GitHub-Flavored Markdown (`# H1` wiki pages under `decisions/`, `_rules/`, `gotchas/`, `concepts/`, `sessions/`). | `AHTL` Epoch Frames (`L3`/`L4`) and `BiTemporalGraph` functional edges (`status`, `exit_code`, `pass`, `fail`, `return_type`) carry structured state across turns. |
| **Raw Byte Provenance** | **100% Lossless (`L0`):** Gear-hash FastCDC (`2 KiB` min, `8 KiB` avg, `64 KiB` max) + 256-bit `BLAKE3` CAS (`#icx:<16-hex>` / `#icx:<12-hex>`). | **Lossy / Summarized:** Raw hook observations are body-capped and compiled into prose summaries. | Every byte of every virtualized tool output and turn is recoverable via `icx deref` or MCP `icx` (`op: "deref"` / `"slice"` / `"grep"`). |
| **In-Flight Tool Virtualization** | **Yes (`< 500 us`):** Intercepts `PostToolUse` / `prune_tool_results_only` for outputs `>= tool_virtualize_threshold`, redacting secrets and replacing bloat with a 40–120 token `⟪ICX:STUB│#icx:...⟫` frame. | **No:** Observes tool calls passively for session logs; never rewrites `updatedToolOutput` or `updatedMCPToolOutput`. | Native `ClaudeCodeAdapter`, `CodexAdapter`, `AntigravityAdapter`, `HermesAdapter`, and `WireProxyTransformer` prevent context window exhaustion. |
| **Associative & Graph Indexing** | **Sub-50us Hardware-Aligned:** 10,240-bit `Hypervector` (`[u64; 160]`), 16-D Contractive IFS `FractalCoord` (`FCGA`), `TitansSurpriseGate`, and CSR `BiTemporalGraph` (`PersonalizedPageRank`). | SQLite FTS5 + lexical entity + RRF ranking over compiled Markdown wiki pages (`memory_query`). | `InfinityContextMmu::build_ephemeral_tail_slot` runs 2-stage HDC + Fractal suffix + 6-step HippoRAG PPR walk on every turn. |
| **Cross-Agent Same-Host IPC** | **Lock-Free `/dev/shm` Seqlock Ring (`< 500 ns`):** `SeqlockTelemetryBus` (`1,048,640` bytes: `64 B` `FabricHeader` + `256 x 4,096 B` `SeqlockSlot` ring). | SQLite-backed session tables and explicit `memory_handoff_*` / `memory_message_*` MCP calls. | Sibling agents on the same workspace receive `⟦Θ:<agent>@<subject>⊢...⟧` telepathy frames automatically in their next turn's Ephemeral Tail Slot. |
| **Prompt Prefix-Cache Protection** | **Yes (`>= 94%` hit rate):** 4-Zone `CacheEpochController` (`Zone 0` Pinned Head, `Zone 1` Sealed Epochs, `Zone 2` Active Window, `Zone 3` Ephemeral Tail Slot) + HTTP Wire Proxy (`127.0.0.1:49380` / `:8787`). | **No:** Does not proxy LLM provider traffic or manage `cache_control` breakpoints. | `WireProxyTransformer` pins Anthropic `cache_control: {"type": "ephemeral"}` and preserves OpenAI 1,024-token prefix blocks. |
| **Pre-Ingestion Security Gate** | 4-Tier `SecretRedactor` (known prefixes, PEM blocks, URL/env credentials, Shannon entropy `> 4.65 bits/char` on `>= 24` char tokens) + AHTL delimiter neutralization before `CasStore`. | Server-side secret scrubbing on observation/message bodies and untrusted-history framing. | Guarantees `0%` secret leakage into `.icx/cas_store.json`, `bitemporal_graph.json`, `mmu_state.json`, or `/dev/shm`. |

### 1.2 Standalone Durable Persistence Architecture (`.icx/` Directory Layout)

When operating in **Mode A (Standalone `icx` without `ai-memory`)**, `InfinityContextMmu::load_workspace_dir` and `InfinityContextMmu::save_workspace_dir` maintain complete cross-session durability inside the repository's `.icx/` directory, coordinated by an OS advisory file lock (`.icx/cas_store.lock` with `0o600` permissions) plus an in-process mutex (`WORKSPACE_IO_LOCK`) and atomic temp-file-and-rename writes (`<filename>.tmp.<pid>.<seq>`):

| File Path | Owning Subsystem | Persisted Contents & Role in Standalone Mode |
| :--- | :--- | :--- |
| `.icx/cas_store.lock` | `InfinityContextMmu::acquire_workspace_lock` | Advisory file lock (`mode 0600`) serializing concurrent CLI, hook, MCP, and proxy transactions across processes. |
| `.icx/cas_store.json` | `icx_core::cas::CasStore` | FastCDC content-defined chunks, 256-bit `BLAKE3` digests, line-offset index tables, and `ArtifactId` metadata (`mode 0600`) for 100% lossless `deref`, `slice`, and cold `grep`. |
| `.icx/bitemporal_graph.json` | `icx_store::BiTemporalGraph` | Bi-temporal edges (`t_valid`, `t_invalid`, `t_tx_start`, `t_tx_end`) with clear-state functional supersession (`status`, `fail`, `pass`, `exit_code`, `return_type`). |
| `.icx/mmu_state.json` | `icx_adapters::mmu::PersistedMmuState` | Full `MerkleContextDag` (`L1` turns, `L2` folded subagent/loop branches, `L3`/`L4` sealed Cache Epochs), `CacheEpochController`, `hdc_events` replay log, `subagent_branches`, `agent_read_cursors`, `indexed_occurrences`, `redacted_ids`, and monotonic logical `clock_ms`. |
| `.icx/icx-fabric-v1.shm` (or `/dev/shm/icx-fabric-v1`) | `icx_bus::SeqlockTelemetryBus` | Fixed `1,048,640`-byte POSIX shared-memory Seqlock ring (`ICX_SHM_MAGIC = 0x4943_585F_4641_4231`, `mode 0600`) storing the 256 most recent cross-agent telepathy frames and `global_dag_root`. |

---

## 2. End-to-End Setup & Configuration Guide (Mode A: Standalone `icx`)

### 2.1 Installation, Workspace Initialization & Environment Variables

#### Step 1: Build and Install the `icx` Binary
Build from source using Rust `1.100+` (`Edition 2024`, permanently `unsafe_code = "forbid"` across all 8 crates):

```bash
cargo install --path crates/icx-cli --locked
which icx
icx help
```

#### Step 2: Initialize the Workspace `.icx/` Store & Verify `/dev/shm` Permissions
Initialize the workspace store and verify that both `.icx/icx-fabric-v1.shm` and `/dev/shm` files are created with strict owner-only `mode 0600` (`-rw-------`) permissions:

```bash
icx init --dir .icx
stat -c "%a %n" .icx/cas_store.json .icx/icx-fabric-v1.shm
```

Expected output:

```text
⟪ICX:INIT│dir=.icx│status=READY⟫
600 .icx/cas_store.json
600 .icx/icx-fabric-v1.shm
```

#### Step 3: Core Environment Variables
Configure the following environment variables in your shell profile (`~/.config/fish/config.fish` or `~/.bashrc`) or per-agent settings:

| Environment Variable | Default / Recommended Value | Purpose |
| :--- | :--- | :--- |
| `ICX_DIR` | `.icx` (or `/path/to/repo/.icx`) | Path to the workspace Content-Addressed Store, Bi-Temporal Graph, and MMU state directory. |
| `ICX_SHM_PATH` | `/dev/shm/icx-fabric-v1` (or `.icx/icx-fabric-v1.shm`) | Path to the `1,048,640`-byte lock-free Seqlock telepathy ring buffer (`mode 0600`). |
| `ANTHROPIC_BASE_URL` | `http://127.0.0.1:49380/anthropic` (or `http://127.0.0.1:8787`) | Routes Claude Code and Anthropic SDK clients through the Cache-Epoch Wire Proxy. |
| `OPENAI_BASE_URL` | `http://127.0.0.1:49380/openai/v1` (or `http://127.0.0.1:8787/v1`) | Routes OpenAI Codex CLI and OpenAI SDK clients through the Cache-Epoch Wire Proxy. |
| `GEMINI_BASE_URL` / `GEMINI_API_BASE_URL` | `http://127.0.0.1:49380/gemini` | Routes Google Antigravity CLI (`agy`) and Gemini CLI traffic through the Cache-Epoch Wire Proxy. |

---

### 2.2 Claude Code (`2.1.326+`) Configuration

Claude Code (`2.1.326+`) is configured via `~/.claude/settings.json` (or project `.claude/settings.json`) for lifecycle hooks and `.mcp.json` for the Single-Tool Polymorphic MCP Pager.

#### Project `.mcp.json`

```json
{
  "mcpServers": {
    "icx": {
      "type": "stdio",
      "command": "icx",
      "args": ["mcp-serve"],
      "env": {
        "ICX_DIR": ".icx"
      }
    }
  }
}
```

#### `~/.claude/settings.json`

```json
{
  "env": {
    "ICX_DIR": ".icx",
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:49380/anthropic"
  },
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SessionStart --dir .icx"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event UserPromptSubmit --dir .icx"
          }
        ]
      }
    ],
    "PreToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PreToolUse --dir .icx"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PostToolUse --dir .icx"
          }
        ]
      }
    ],
    "SubagentStart": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SubagentStart --dir .icx"
          }
        ]
      }
    ],
    "SubagentStop": [
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SubagentStop --dir .icx"
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PreCompact --dir .icx"
          }
        ]
      }
    ],
    "PostCompact": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PostCompact --dir .icx"
          }
        ]
      }
    ],
    "Stop": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event Stop --dir .icx"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SessionEnd --dir .icx"
          }
        ]
      }
    ]
  }
}
```

---

### 2.3 OpenAI Codex CLI (`0.169.0+`) Configuration

OpenAI Codex (`0.169.0+`) requires `[features] codex_hooks = true` and `[mcp_servers.icx]` in `~/.codex/config.toml`, plus lifecycle hooks in `~/.codex/hooks.json`.

#### `~/.codex/config.toml`

```toml
model = "gpt-5-codex"
model_provider = "icx_openai"

[features]
codex_hooks = true

[model_providers.icx_openai]
name = "OpenAI via Infinity-Context Cache-Epoch Proxy"
base_url = "http://127.0.0.1:49380/openai/v1"
env_key = "OPENAI_API_KEY"
wire_api = "responses"

[mcp_servers.icx]
command = "icx"
args = ["mcp-serve"]
env = { ICX_DIR = ".icx" }
```

To run a dedicated local proxy instance on port `8787` (or the default `49380`) for Codex:

```bash
icx proxy --listen 127.0.0.1:8787 --provider openai --dir .icx
```

#### `~/.codex/hooks.json`

```json
{
  "hooks": {
    "session-start": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "session-start", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "user-prompt-submit": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "user-prompt-submit", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "pre-tool-use": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "pre-tool-use", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "post-tool-use": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "post-tool-use", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "subagent-start": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "subagent-start", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "subagent-stop": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "subagent-stop", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "pre-compact": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "pre-compact", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "post-compact": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "post-compact", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ],
    "session-end": [
      {
        "command": ["icx", "hook", "--agent", "codex", "--event", "session-end", "--dir", ".icx"],
        "timeout_ms": 2000
      }
    ]
  }
}
```

---

### 2.4 Google Antigravity CLI (`agy`) Configuration

Google Antigravity CLI (`agy`) reads hook definitions from `~/.gemini/antigravity-cli/hooks.json`, `~/.gemini/config/hooks.json`, or project-local `.agents/hooks.json`, and MCP definitions from `~/.gemini/antigravity-cli/mcp_config.json`.

#### `~/.gemini/antigravity-cli/hooks.json` (and `.agents/hooks.json`)

```json
{
  "hooks": {
    "SessionStart": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "SessionStart", "--dir", ".icx"]
      }
    ],
    "PreInvocation": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "PreInvocation", "--dir", ".icx"]
      }
    ],
    "PostInvocation": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "PostInvocation", "--dir", ".icx"]
      }
    ],
    "PreToolUse": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "PreToolUse", "--dir", ".icx"]
      }
    ],
    "PostToolUse": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "PostToolUse", "--dir", ".icx"]
      }
    ],
    "SubagentStart": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "SubagentStart", "--dir", ".icx"]
      }
    ],
    "SubagentStop": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "SubagentStop", "--dir", ".icx"]
      }
    ],
    "Stop": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "Stop", "--dir", ".icx"]
      }
    ],
    "SessionEnd": [
      {
        "command": "icx",
        "args": ["hook", "--agent", "agy", "--event", "SessionEnd", "--dir", ".icx"]
      }
    ]
  }
}
```

#### `~/.gemini/antigravity-cli/mcp_config.json`

```json
{
  "mcpServers": {
    "icx": {
      "command": "icx",
      "args": ["mcp-serve"],
      "env": {
        "ICX_DIR": ".icx"
      }
    }
  }
}
```

---

### 2.5 Hermes Agent Configuration (`ContextEngine`, `MemoryProvider` & `hooks.yaml`)

#### `~/.hermes/hooks.yaml` (and `cli-config.yaml`)

```yaml
context:
  engine: infinity_context
  protect_first_n: 1
  protect_last_n: 2

memory:
  provider: infinity_context

hooks:
  SessionStart:
    - command: ["icx", "hook", "--agent", "hermes", "--event", "SessionStart", "--dir", ".icx"]
  PostToolUse:
    - command: ["icx", "hook", "--agent", "hermes", "--event", "on_turn_complete", "--dir", ".icx"]
  PreCompact:
    - command: ["icx", "hook", "--agent", "hermes", "--event", "on_pre_compress", "--dir", ".icx"]

mcp_servers:
  icx:
    command: icx
    args: ["mcp-serve"]
    env:
      ICX_DIR: ".icx"
```

#### Python `ContextEngine` & `MemoryProvider` Subclass Wiring (`plugins/context_engine/infinity_context/__init__.py`)

```python
"""Hermes ContextEngine and MemoryProvider subclasses wired to Infinity-Context (icx v1.0.0)."""

from __future__ import annotations

import json
import subprocess
from typing import Any

from agent.context_engine import ContextEngine
from agent.memory_provider import MemoryProvider

PRE_COMPRESS_CHECKPOINT_API_VERSION = 2


class InfinityContextEngine(ContextEngine):
    """Replaces middle-history messages with Cache-Epoch AHTL frames without mutating state.db."""

    def __init__(self, protect_first_n: int = 1, protect_last_n: int = 2, icx_dir: str = ".icx") -> None:
        self.protect_first_n = protect_first_n
        self.protect_last_n = protect_last_n
        self.icx_dir = icx_dir

    def _call_icx(self, event: str, payload: dict[str, Any]) -> dict[str, Any]:
        proc = subprocess.run(
            ["icx", "hook", "--agent", "hermes", "--event", event, "--dir", self.icx_dir],
            input=json.dumps(payload),
            text=True,
            capture_output=True,
            check=False,
        )
        if proc.returncode != 0 or not proc.stdout.strip():
            return {}
        return json.loads(proc.stdout)

    def select_context(
        self,
        request_messages: list[dict[str, Any]],
        *,
        conversation_messages: list[dict[str, Any]] | None = None,
        incoming_message: str | None = None,
        budget_tokens: int | None = None,
    ) -> list[dict[str, Any]]:
        resp = self._call_icx(
            "select_context",
            {
                "request_messages": request_messages,
                "protect_first_n": self.protect_first_n,
                "protect_last_n": self.protect_last_n,
            },
        )
        return resp.get("messages", request_messages)

    def prune_tool_results_only(
        self,
        messages: list[dict[str, Any]],
        current_tokens: int | None = None,
    ) -> tuple[list[dict[str, Any]], int]:
        resp = self._call_icx(
            "prune_tool_results_only",
            {
                "messages": messages,
                "protect_last_n": self.protect_last_n,
            },
        )
        return resp.get("messages", messages), int(resp.get("pruned_count", 0))

    def compress(
        self,
        messages: list[dict[str, Any]],
        current_tokens: int | None = None,
        focus_topic: str | None = None,
        force: bool = False,
        memory_context: str | None = None,
    ) -> list[dict[str, Any]]:
        resp = self._call_icx(
            "compress",
            {
                "messages": messages,
                "focus_topic": focus_topic,
            },
        )
        return resp.get("messages", messages)

    def on_turn_complete(
        self,
        messages: list[dict[str, Any]],
        usage: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> int:
        resp = self._call_icx("on_turn_complete", {"messages": messages})
        return int(resp.get("ingested_count", 0))


class InfinityContextMemoryProvider(MemoryProvider):
    """Fail-closed v2 pre-compression checkpoint handoff to Infinity-Context."""

    def __init__(self, icx_dir: str = ".icx") -> None:
        self.icx_dir = icx_dir

    def on_pre_compress(self, messages: list[dict[str, Any]]) -> str:
        proc = subprocess.run(
            ["icx", "hook", "--agent", "hermes", "--event", "on_pre_compress", "--dir", self.icx_dir],
            input=json.dumps({"messages": messages}),
            text=True,
            capture_output=True,
            check=True,
        )
        data = json.loads(proc.stdout)
        if data.get("checkpoint_version") != PRE_COMPRESS_CHECKPOINT_API_VERSION:
            raise RuntimeError("icx checkpoint API version mismatch")
        return str(data["checkpoint"])
```

---

### 2.6 Generic MCP & ACP Harnesses (Cursor, Windsurf, Cline, OpenCode, Gemini CLI)

Any MCP-compliant editor or CLI connects via the single-tool `icx` stdio server (`< 180` tokens of schema overhead) and optional HTTP Wire Proxy (`ANTHROPIC_BASE_URL` or `OPENAI_BASE_URL`).

---

## 3. Dual-Stack Integration (`icx` + `ai-memory` Coexistence Architecture — Mode B)

In **Mode B**, **Infinity-Context (`icx`)** and **`ai-memory`** run side by side across all harnesses. Their responsibilities are strictly orthogonal:
- **`icx`** owns the sub-500us machine-native Virtual Context MMU: `PostToolUse` output stubbing (`#icx:<16-hex>`), lossless BLAKE3 FastCDC CAS, 10,240-bit HDC + Fractal + Bi-Temporal PPR working recall, `/dev/shm` Seqlock telepathy, and Cache-Epoch wire proxying.
- **`ai-memory`** owns the human-curated semantic Markdown Wiki (`decisions/`, `_rules/`, `gotchas/`, `procedures/`, `concepts/`, `profile/`), single-use cross-session handoffs (`memory_handoff_*`), and cross-project agent inbox messaging (`memory_message_*`).

### 3.1 Lifecycle Hook Ordering & Non-Collision Rules

Claude Code (`settings.json`), OpenAI Codex (`hooks.json`), and Google Antigravity CLI (`hooks.json`) execute hook arrays **sequentially in declaration order**:

1. **`SessionStart` (`ai-memory` first, `icx` second):**
   - `ai-memory` runs first to consume any single-use pending handoff (`memory_handoff_accept`) and surface pinned project rules (`_rules/`, `decisions/`, `profile/`) plus any waiting cross-project inbox count.
   - `icx` runs second to inject `ICX_AHTL_LEGEND` and the workspace's initial `⟪ICX:TAIL_SLOT│...⟫` (which is UI-hidden in `agy` via `metadata.user_markdown: ""`).
2. **`UserPromptSubmit` / `PreInvocation` (`icx` first, `ai-memory` second):**
   - `icx` runs its `< 250 us` 10,240-bit HDC + Fractal + Bi-Temporal PPR + `/dev/shm` telepathy poll on every turn.
   - `ai-memory` handles `reload context` compaction recaps or prompt capture without duplicating `icx`'s sub-millisecond structural recall.
3. **`PostToolUse` (`icx` first for virtualization, `ai-memory` second for observation):**
   - `icx` intercepts bulky raw tool outputs (`>= 512` bytes default / `>= 2,048` bytes configurable), redacts secrets, commits lossless bytes to `.icx/cas_store.json`, and emits `updatedToolOutput` / `updatedMCPToolOutput` (`⟪ICX:STUB│#icx:<16-hex>│...⟫`).
   - **MCP Tool Exemption Rule:** `InfinityContextMmu::is_icx_pager_tool` always exempts `icx` (`mcp__icx__icx`) so dereferenced pages reach the model verbatim. To also exempt `ai-memory` wiki reads (`mcp__ai-memory__memory_read_page`, etc.) from `PostToolUse` stubbing in Claude Code, set `icx`'s `PostToolUse` matcher to `^(?!mcp__ai[-_]memory__).+`.
4. **`PreCompact`, `PostCompact`, `Stop` & `SessionEnd` (`icx` first, `ai-memory` second):**
   - `icx` folds open subagent branches and seals the lossless `L3` `CacheEpochNode` first, then `ai-memory` records the session boundary or queues background wiki consolidation.

```mermaid
sequenceDiagram
    participant Agent as Agent Harness (Claude / Codex / AGY)
    participant ICX as icx hook (<500us Rust MMU)
    participant Shm as /dev/shm/icx-fabric-v1 + .icx/
    participant AIM as ai-memory hook / MCP (127.0.0.1:49375)

    Note over Agent,AIM: 1. SessionStart
    Agent->>AIM: SessionStart stdin JSON
    AIM-->>Agent: Consumed single-use handoff + pinned _rules/ & decisions/
    Agent->>ICX: SessionStart stdin JSON
    ICX->>Shm: Poll active Cache Epochs & sibling agent telepathy
    ICX-->>Agent: ICX_AHTL_LEGEND + Ephemeral Tail Slot (UI-hidden in AGY)

    Note over Agent,AIM: 2. PostToolUse (Bulky Tool Output >= Threshold)
    Agent->>ICX: PostToolUse (e.g. Bash cargo test, 84 KB output)
    ICX->>Shm: Redact secrets -> FastCDC/BLAKE3 CAS (#icx:a8f02b91) -> Publish Seqlock slot
    ICX-->>Agent: updatedToolOutput = "⟪ICX:STUB│#icx:a8f02b91│raw=84210B→312B...⟫"
    Agent->>AIM: PostToolUse observation capture

    Note over Agent,AIM: 3. Deliberate Knowledge Promotion (End of Task)
    Agent->>ICX: MCP `icx` (op="deref" / "slice", id="#icx:a8f02b91")
    ICX-->>Agent: Exact lossless lines verifying root cause
    Agent->>AIM: MCP `memory_write_page` (decisions/csr-ppr-fix.md, pinned=true)
    Agent->>AIM: MCP `memory_handoff_begin` (next-session summary)
```

#### Dual-Stack `~/.claude/settings.json` Example (Mode B)

```json
{
  "env": {
    "ICX_DIR": ".icx",
    "ANTHROPIC_BASE_URL": "http://127.0.0.1:49380/anthropic"
  },
  "mcpServers": {
    "icx": {
      "command": "icx",
      "args": ["mcp-serve"],
      "env": {
        "ICX_DIR": ".icx"
      }
    },
    "ai-memory": {
      "command": "$HOME/.local/lib/ai-memory/ai-memory",
      "args": ["mcp-bridge"]
    }
  },
  "hooks": {
    "SessionStart": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.local/lib/ai-memory/ai-memory hook claude-code SessionStart"
          },
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SessionStart --dir .icx"
          }
        ]
      }
    ],
    "UserPromptSubmit": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event UserPromptSubmit --dir .icx"
          },
          {
            "type": "command",
            "command": "$HOME/.local/lib/ai-memory/ai-memory hook claude-code UserPromptSubmit"
          }
        ]
      }
    ],
    "PostToolUse": [
      {
        "matcher": "^(?!mcp__ai[-_]memory__).+",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PostToolUse --dir .icx"
          }
        ]
      },
      {
        "matcher": ".*",
        "hooks": [
          {
            "type": "command",
            "command": "$HOME/.local/lib/ai-memory/ai-memory hook claude-code PostToolUse"
          }
        ]
      }
    ],
    "PreCompact": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PreCompact --dir .icx"
          },
          {
            "type": "command",
            "command": "$HOME/.local/lib/ai-memory/ai-memory hook claude-code PreCompact"
          }
        ]
      }
    ],
    "PostCompact": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event PostCompact --dir .icx"
          }
        ]
      }
    ],
    "SessionEnd": [
      {
        "matcher": "",
        "hooks": [
          {
            "type": "command",
            "command": "icx hook --agent claude-code --event SessionEnd --dir .icx"
          },
          {
            "type": "command",
            "command": "$HOME/.local/lib/ai-memory/ai-memory hook claude-code SessionEnd"
          }
        ]
      }
    ]
  }
}
```

### 3.2 Side-by-Side `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` Instruction Layout

When configuring instruction files for Dual-Stack Mode B, place `icx` operational rules outside `ai-memory`'s managed marker block (`<!-- ai-memory:start -->` ... `<!-- ai-memory:end -->`) so `memory_install_self_routing` refreshes never overwrite `icx` guidance:

```markdown
# Project Agent Instructions (AGENTS.md)

## Infinity-Context (`icx`) Virtual Context MMU Protocol
- Bulky tool outputs are automatically virtualized into AHTL Holographic Stubs (`⟪ICX:STUB│#icx:<16-hex>│...⟫`).
- NEVER re-run a build/test command or re-read an entire file just to inspect output that was stubbed with an `#icx:<hex>` handle.
- Instead, invoke the `icx` MCP tool:
  - `{"op": "slice", "id": "#icx:<hex>", "range": [start_line, end_line]}` for targeted line inspection.
  - `{"op": "grep", "id": "#icx:<hex>", "q": "<regex>"}` to search inside a cold artifact (or omit `id` to search all workspace artifacts).
  - `{"op": "deref", "id": "#icx:<hex>"}` for 100% byte-exact reconstruction.
  - `{"op": "recall", "q": "<query>"}` for 10,240-bit HDC + Fractal + Bi-Temporal PPR associative recall.
  - `{"op": "xagent"}` to inspect live `/dev/shm` telepathy commits from sibling agents.
  - `{"op": "fold", "q": "begin:<label>"}` and `{"op": "fold", "q": "<summary>"}` to fold resolved debug loops.

<!-- ai-memory:start -->
<!-- Managed by ai-memory routing-install skill; do not edit inside these markers -->
<!-- ai-memory:end -->
```

### 3.3 Division of Labor: `icx` MCP Operations vs. `ai-memory-*` Skills

| Operation / Skill | System | Trigger & Latency | Exact When-to-Use Rule in Mode B |
| :--- | :--- | :--- | :--- |
| `icx` (`op: "deref"`) | `icx` MCP | Model page fault (`< 300 us`) | Reconstruct 100% byte-identical raw text of any `#icx:<16-hex>` or `#icx:<12-hex>` handle from `CasStore` (`L0`) or `MerkleContextDag` (`L1`–`L4`). |
| `icx` (`op: "slice"`) | `icx` MCP | Model line slice (`< 200 us`) | Fetch an exact 1-based inclusive line range `range: [start, end]` from a stubbed build log, test run, or file diff without loading the full artifact. |
| `icx` (`op: "grep"`) | `icx` MCP | Cold storage regex (`< 500 us`) | Run regex/literal search `q` across a specific `#icx:<hex>` artifact or across all cold artifacts in `.icx/cas_store.json`. |
| `icx` (`op: "recall"`) | `icx` MCP | Associative/graph query (`< 250 us`) | Query 10,240-bit HDC Hamming similarity + 16-D Fractal trajectory suffix + 6-step HippoRAG `PersonalizedPageRank` over functional graph edges. |
| `icx` (`op: "fold"` / `"expand"`) | `icx` MCP | Sub-task boundary (`< 100 us`) | Call `q: "begin:<label>"` before an iterative debug/test loop, `q: "<outcome_summary>"` afterward to fold into an `L2` `SubTrajectoryBranch` handle, and `deref`/`slice` on the branch handle to expand child turns. |
| `icx` (`op: "xagent"`) | `icx` MCP | Live telepathy poll or broadcast (`< 100 us`) | Poll the latest `k` (`1..=256`, default `10`) live cross-agent AHTL frames (`⟦Θ:<agent>@<subj>⊢...⟧`) from `/dev/shm/icx-fabric-v1` (when `q` is omitted) or broadcast a new telepathy frame (`q` non-empty, optional subject `id`). |
| `icx` (`op: "status"`) | `icx` MCP | MMU telemetry (`< 50 us`) | Check `cas_artifacts`, `unique_chunks`, `dag_nodes`, `sealed_epochs`, `hdc_entries`, `graph_edges`, and `shm_seq`. |
| `ai-memory-retrieval` | `ai-memory` Skill | Pre-design / catch-up (`memory_query`, `memory_read_page`, `memory_briefing`) | Query human-curated project rules (`_rules/`), ADRs (`decisions/`), known pitfalls (`gotchas/`), or recover a cut-off session's observations (`memory_read_session_observations` with `order: "desc"`). |
| `ai-memory-durable-pages` | `ai-memory` Skill | Explicit user request (`memory_write_page`, `memory_delete_page`, `memory_restore_page`) | Persist vetted architectural decisions (`decisions/<slug>.md`, `pinned: true`), project conventions (`_rules/<slug>.md`), or cross-project operator preferences (`scope: "profile"`). |
| `ai-memory-handoff` | `ai-memory` Skill | Session wrap-up / resume (`memory_handoff_begin`, `memory_handoff_accept`, `memory_handoff_list`) | Pass a concise 2–3 sentence baton + open questions/next steps to the next session when ending work. |
| `ai-memory-learning-maintenance` | `ai-memory` Skill | Wiki hygiene (`memory_consolidate`, `memory_auto_improve`, `memory_lint`, `memory_forget_sweep`) | Audit the Markdown wiki for contradictions (`feedback_flagged`), prune expired TTL notes, or review completed sessions for rule proposals. |
| `ai-memory-messaging` | `ai-memory` Skill | Cross-repository delegation (`memory_message_send`, `memory_message_list`, `memory_message_pop`) | Send a self-contained task request to a **different** project's inbox (`to_workspace` + `to_project`) without loading that repo into the current session. |
| `ai-memory-routing-install` | `ai-memory` Skill | Setup/refresh (`memory_install_self_routing`) | Install or refresh `<!-- ai-memory:start -->` instruction blocks and `<!-- ai-memory-managed: routing-skill -->` skill files without touching `icx` sections. |

### 3.4 Port, Socket, and Storage Isolation Table

| Resource Type | Infinity-Context (`icx` `v1.0.0`) | `ai-memory` Daemon & Bridge | Conflict Status |
| :--- | :--- | :--- | :--- |
| **Loopback HTTP Port** | `127.0.0.1:49380` / `127.0.0.1:8787` (`icx proxy`) | `127.0.0.1:49375` (`axum` REST + MCP server) | **Zero conflict** (distinct ports). |
| **MCP Stdio Transport** | `icx mcp-serve` (`stdio` JSON-RPC 2.0) | `ai-memory mcp-bridge` (`stdio` -> `127.0.0.1:49375`) | **Zero conflict** (independent processes). |
| **POSIX Shared Memory** | `/dev/shm/icx-fabric-v1` & `.icx/icx-fabric-v1.shm` (`1,048,640` bytes, `mode 0600`) | None | **Zero conflict**. |
| **Workspace / Local Storage** | `<repo>/.icx/` (`cas_store.json`, `bitemporal_graph.json`, `mmu_state.json`, `cas_store.lock`) | `~/.local/share/ai-memory/` (`memory.sqlite`, `wiki/<workspace>/<project>/*.md`) + optional `<repo>/.ai-memory.toml` | **Zero conflict** (disjoint paths). |
| **MCP Server Registration** | `"icx"` (`icx mcp-serve` -> single tool `icx`) | `"ai-memory"` (`ai-memory mcp-bridge` -> `memory_*` tools) | **Zero conflict** (disjoint namespaces). |
| **Instruction File Markers** | Outside `ai-memory` markers in `AGENTS.md` / `CLAUDE.md` / `GEMINI.md` | Strictly confined between `<!-- ai-memory:start -->` and `<!-- ai-memory:end -->` | **Zero conflict** (Managed-Block isolation). |

---

## 4. Co-Existence with Native Agent Context Windows & Compaction Behavior

A primary engineering requirement of `Infinity-Context` is **cooperative co-existence** with each agent harness's native context window manager, prompt-caching logic, and compaction lifecycle. Rather than fighting the harness or invalidating its KV cache on every turn, `icx` shapes the payload *before* it enters the model request and intercepts compaction boundaries so zero raw information is ever lost.

### 4.1 Claude Code (`2.1.326+`): 4-Breakpoint Cache Budget, `PostToolUse` Rewriting & `PreCompact` Control

Claude Code manages a 200K–1M token window backed by Anthropic's 4-breakpoint `cache_control: {"type": "ephemeral"}` prompt cache and an auto-compactor that triggers when context utilization crosses ~92%.

1. **Anthropic 4-Breakpoint `cache_control` Budget Allocation:**
   Anthropic allows at most **4** `cache_control` breakpoints per `/v1/messages` request. `WireProxyTransformer::transform_anthropic_messages` and Claude Code cooperate over those 4 slots as follows:
   - **Breakpoint 1 (Zone 0 — System Prompt):** Pinned on the static system prompt + `ICX_AHTL_LEGEND`.
   - **Breakpoint 2 (Zone 0 — Tool Definitions):** Pinned on the MCP + built-in tool schema array (kept tiny by `icx`'s single-tool polymorphic schema `< 180` tokens).
   - **Breakpoint 3 (Zone 1 — Sealed AHTL Cache Epoch `E_{k-1}`):** Pinned by `WireProxyTransformer` (`msg["cache_control"] = {"type": "ephemeral"}`) on the final message of the sealed middle-history epoch (`fold_end - 1`), while removing stale `cache_control` markers from earlier messages (`0..fold_end - 1`).
   - **Breakpoint 4 (Reserved / Active Window Anchor):** Left available for Claude Code's native `requestJournal` (`keepsCachedPrefix`) or multi-turn active window pinning.
2. **In-Flight `PostToolUse` Output Virtualization (`updatedToolOutput` & `updatedMCPToolOutput`):**
   - Before a bulky tool output (`>= 512` bytes default, or `>= 2,048` bytes when tuned for large-window profiles) ever enters Claude Code's transcript array, `ClaudeCodeAdapter` intercepts `PostToolUse` and emits `updatedToolOutput` (for all built-in tools like `Bash`, `Read`, `Grep`) and `updatedMCPToolOutput` (when `tool_name.starts_with("mcp__")`).
   - Because the replacement happens *before* the tool result is appended to the conversation history, Claude Code's context window grows by ~60 tokens per tool call instead of 20,000+ tokens, delaying native auto-compaction by **10x–25x**.
3. **Ephemeral Tail Slot Injection (`UserPromptSubmit`):**
   - On `UserPromptSubmit`, `ClaudeCodeAdapter` returns `hookSpecificOutput.additionalContext` containing `⟪ICX:TAIL_SLOT│agent=claude-code│...⟫` (or `<icx-ephemeral-tail-slot>`). Because this block is attached at the very tail of the current turn (`Zone 3`), `Zone 0` and `Zone 1` remain 100% byte-identical across turns.
4. **Co-Existence with `PreCompact` & `PostCompact`:**
   - When Claude Code eventually reaches `/compact` (manual or auto), `PreCompact` fires first.
   - **Mode 1 — Lossless Checkpoint + Allow Native Compaction (Default):** `ClaudeCodeAdapter` calls `mmu.rollover_and_sync_dag_root()` to seal all `L1` turns into an immutable `L3 CacheEpochNode` in `.icx/`, then `PostCompact` re-injects `win.deterministic_prefix()` via `systemMessage` so every `#icx:<16-hex>` handle, function signature, and panic location survives compaction intact.
   - **Mode 2 — Blocking Lossy Native Compaction (Exit Code `2`):** In Claude Code `2.1.326+`, if a `PreCompact` hook exits with status code `2` (or returns a block decision when `WireProxyTransformer` is already folding middle turns into AHTL epochs on the wire), Claude Code aborts its lossy LLM summarizer (`"Compaction blocked by PreCompact hook; continuing uncompacted"`) and continues using `icx-proxy`'s deterministic Cache-Epoch folding.

---

### 4.2 OpenAI Codex CLI (`0.169.0+`): `/v1/responses` Prefix Caching, `updatedMCPToolOutput` vs. Native Tools & `CodexRolloutTailer`

OpenAI Codex (`0.169.0+`) uses OpenAI's `/v1/responses` API with automatic 1,024-token prefix caching (and optional `previous_response_id` state chaining) plus an internal Rust tool executor.

1. **Why `PostToolUse` Behaves Differently for Built-in vs. MCP Tools in Codex:**
   - Forensic inspection of the `codex` `0.169.0` binary (`ev_7cda5928199ea58224bdedbbd14892ff224515db64f7679457f633d5e7d816d3`) revealed that Codex's `post-tool-use.command.output` JSON schema (`additionalProperties: false`) accepts `hookSpecificOutput.updatedMCPToolOutput`, which replaces the output of **MCP tools** (`tool_name.starts_with("mcp__")`), but is a **no-op for built-in Rust tools** (`shell`, `unified_exec`, `apply_patch`).
   - Therefore, `CodexAdapter::handle_hook("post-tool-use", ...)` branches on `tool_name`:
     - If `tool_name.starts_with("mcp__")` and `res.was_virtualized`: emits `{"hookEventName": "PostToolUse", "updatedMCPToolOutput": stub}` to replace the bulky MCP output in-flight.
     - If `tool_name` is a built-in tool (such as `shell` / `Bash`): ingests the raw output into `.icx/cas_store.json`, publishes the telepathy frame to `/dev/shm`, and emits `{"hookEventName": "PostToolUse", "additionalContext": res.compressed_ahtl}` containing the `#icx:<16-hex>` handle and high-surprise diagnostic clauses.
2. **Wire-Level Middle-History Virtualization (`WireProxyTransformer::transform_openai_request`):**
   - Because Codex's built-in `shell` outputs are not truncated by `post-tool-use` alone, pointing Codex to `icx-proxy` (`OPENAI_BASE_URL=http://127.0.0.1:49380/openai/v1` or `127.0.0.1:8787`) allows `WireProxyTransformer::transform_openai_request` to inspect the `/v1/responses` `input` array (`function_call_output` items and `input_text` parts).
   - Any middle-history item outside `protect_tail = 2` with length `>= tool_virtualize_threshold` is deterministically replaced with its AHTL packet while preserving `system`/`developer` prompts (`Zone 0`) verbatim, keeping OpenAI's automatic 1,024-token prefix cache above `94%` hit rate.
3. **Out-of-Band JSONL Tailing (`CodexRolloutTailer`):**
   - Even when Codex runs with stateful `previous_response_id` or without proxy interception, `CodexRolloutTailer` watches `~/.codex/sessions/**/rollout-*.jsonl` (discovered via `~/.codex/state_5.sqlite` `threads.rollout_path`) using bounded $O(1)$ 16 KiB BLAKE3 anchor digests and ingests every `RolloutItemWire` (`RetainedContext`, `Compacted`, tool outputs) into `.icx/` and `/dev/shm` in real time.

---

### 4.3 Google Antigravity CLI (`agy`): `<CONTEXT_SUMMARY>` Co-Existence, `transcriptPath` Tailing & `Stop` Continuation

Google Antigravity CLI (`agy`) stores trajectories in `~/.gemini/antigravity-cli/conversations/<id>.db` and `<appDataDir>/brain/<id>/.system_generated/logs/transcript.jsonl` (`transcript_full.jsonl`), and periodically truncates older steps into a `<CONTEXT_SUMMARY>` block or halts on `MAX_TOKEN_BUDGET_EXCEEDED` / `MAX_INVOCATIONS`.

1. **UI-Silent `PreInvocation` & `PostInvocation` Injection:**
   - Before every model call (`PreInvocation`), `AntigravityAdapter` injects `injectSteps[].systemMessage` with `"metadata": {"user_markdown": ""}`.
   - If `agy` has truncated earlier turns into `<CONTEXT_SUMMARY>`, the model still receives the exact `#icx:<16-hex>` handles, HDC associative matches (`⟦Ψ:HDC_RECALL⊢...⟧`), and Bi-Temporal PPR provenance (`⟦Ω:PPR_GRAPH⊢...⟧`) in the UI-silent `systemMessage`, and can recover any truncated step byte-for-byte via the `icx` MCP tool.
   - On `PostInvocation`, if `forceContinue: true` is present in the hook payload, `AntigravityAdapter` sets `"terminationBehavior": "force_continue"`.
2. **`PreToolUse` `overwrite` & `PostToolUse` `transcriptPath` Tailing:**
   - `PreToolUse` supports returning `{"decision": "allow", "overwrite": {...}}` when command arguments need bounding.
   - On `PostToolUse`, `agy` frequently passes metadata (`toolCall`, `stepIdx`, `transcriptPath`) without duplicating a multi-megabyte `toolResponse` on hook `stdin`. `AntigravityAdapter::read_transcript_step` automatically seeks to the last `512 KiB` of `transcriptPath` (`transcript_full.jsonl`), locates `stepIdx`, ingests the full output into `.icx/cas_store.json`, and publishes the telepathy event to `/dev/shm`.
3. **`SubagentStart` / `SubagentStop` & Infinite-Horizon `Stop` Rollover:**
   - Subagents launched via `invoke_subagent` are isolated in `mmu.subagent_branches` on `SubagentStart` and folded into a single `L2` `⟪ICX:SUBAGENT_FOLDED│#icx:...⟫` node on `SubagentStop`.
   - When a long-running `agy` session hits `terminationReason == "MAX_TOKEN_BUDGET_EXCEEDED"` or `"MAX_INVOCATIONS"` (and `!continuationActive`), `AntigravityAdapter` folds open subagents, seals the current epoch (`force_rollover_and_sync_dag_root()`), and returns `{"decision": "continue", "reason": "⟪ICX:EPOCH_FOLDED│#icx:...⟫"}` so marathon executions continue seamlessly.

---

### 4.4 Hermes Agent: `ContextEngine` Preservation Masks & Non-Destructive SQLite History

Hermes Agent persists full conversation transcripts in `~/.hermes/state.db` (`sessions` and `messages` tables) and calls `ContextEngine.select_context()` on every outbound LLM request right before prompt-cache derivation.

1. **Zero-Mutation `state.db` Guarantee:**
   - `HermesAdapter::select_context` transforms only the in-memory `request_messages` list for the current API call; Hermes's SQLite `messages` table remains 100% untouched.
2. **Protected Head/Tail Slices & `build_preserve_mask` Invariants:**
   - `select_context(request_messages, protect_first_n, protect_last_n)` keeps the first `protect_first_n` system messages (`Zone 0`) and the last `protect_last_n` active turns (`Zone 2`) verbatim.
   - Across the middle slice (`head_end..tail_start`), `HermesAdapter::build_preserve_mask` enforces three strict protocol-safety invariants so OpenAI/Anthropic API validators never reject a folded message array:
     - **Pager Exemption:** Any message whose subject matches `InfinityContextMmu::is_icx_pager_tool` is preserved verbatim.
     - **Multimodal Safety:** Any message containing structured non-text content parts (`image_url`, audio, binary attachments) is preserved verbatim (`has_structured_non_text_content`).
     - **Atomic `tool_calls` Group Preservation:** Any `assistant` message with a `tool_calls` array and its contiguous block of `role: "tool"` (or `tool_call_id`) response messages are kept together as an indivisible group whenever any member of that group is preserved or when the group straddles the `tail_start` boundary into `protect_last_n`.
   - All remaining middle-slice messages are folded into a single deterministic `role: "system"`, `name: "icx_cache_epoch"` message (`⟪ICX:HERMES_EPOCH│folded_msgs=<N>⟫`), while the dynamic recall block is appended at the end as `role: "system"`, `name: "icx_tail_slot"` (`⟪ICX:TAIL_SLOT│...⟫`).
3. **Idempotent Occurrence Tracking Across Turns (`ingest_or_reuse_at_index`):**
   - Because `select_context` and `on_turn_complete` see the conversation history on every turn, `InfinityContextMmu::ingest_or_reuse_at_index` caches `(agent, clean_subject, ArtifactId, exit_code, force_anomaly)` per message position `occurrence_idx`. Unchanged historical messages reuse their cached `IngestResult` in $O(1)$ without duplicating DAG nodes or `/dev/shm` broadcasts, whereas new message positions execute full ingestion so state transitions (`PASS -> FAIL -> PASS`) update `BiTemporalGraph` accurately.

---

## 5. Recommendations & Best Practices for Use

### 5.1 Tuning `tool_virtualize_threshold` and `epoch_token_threshold` by Provider & Model

| Target Provider & Model Family | Recommended `tool_virtualize_threshold` | Recommended `epoch_rollover_tokens` | `protect_last_n_turns` | `tail_slot_max_tokens` | Rationale |
| :--- | ---: | ---: | ---: | ---: | :--- |
| **Anthropic Claude Opus 4.6 / Sonnet 4.6 (`200K`–`1M` context)** | `2,048` bytes (or `512` B for strict token economy) | `16,384` tokens | `6` turns | `1,024` tokens | Aligns sealed `L3` epochs (`Zone 1`) with Anthropic's 4-breakpoint `cache_control: {"type": "ephemeral"}` minimum cacheable block sizes (1,024–4,096 tokens) and maximizes prefix reuse. |
| **OpenAI GPT-5 / Codex / `o3` / `o4-mini` (`/v1/responses`)** | `1,024` bytes | `8,192` tokens | `4` turns | `1,024` tokens | Aligns with OpenAI's automatic 1,024-token prefix cache granularity (which caches in 128-token increments after 1,024 tokens without explicit breakpoint limits). |
| **Google Gemini (`agy` / `1M`–`2M` context)** | `2,048` bytes | `16,384` tokens | `6` turns | `1,024` tokens | Prevents `MAX_TOKEN_BUDGET_EXCEEDED` during subagent fan-out and keeps `PreInvocation` latency `< 300 us`. |
| **Local / Open-Weight Models (`vLLM`, `llama.cpp`, `32K`–`128K` KV RAM)** | `512` bytes (`DEFAULT_TOOL_VIRTUALIZE_THRESHOLD_BYTES`) | `4,096` tokens | `2` turns | `512` tokens | Aggressively bounds VRAM KV-cache footprint on local GPUs (e.g., RTX 5080 16GB) while retaining 100% lossless CAS dereferencing. |

### 5.2 Single-Agent vs. Multi-Agent Swarm Best Practices

- **Single-Agent Workflows:**
  - Enable native lifecycle hooks (`PostToolUse` + `UserPromptSubmit`/`PreInvocation` + `PreCompact`/`Stop`) and the `icx` MCP server.
  - Point `.icx/` to the repository's local `.icx` directory (`ICX_DIR=.icx`) so CAS artifacts and the Bi-Temporal Graph stay scoped to the repository.
  - Use `icx(op="fold", q="begin:<subtask>")` and `icx(op="fold", q="<summary>")` around iterative compile-fix-test loops to collapse 15+ intermediate turns into a single 50-token `L2` `SubTrajectoryBranch` node.
- **Multi-Agent Swarm Workflows (e.g., Claude Code + Codex + Antigravity CLI):**
  - Point all concurrent agents working on the same repository to the **same** `--dir <REPO>/.icx` directory.
  - `InfinityContextMmu::acquire_workspace_lock` serializes disk updates to `.icx/cas_store.json`, `.icx/bitemporal_graph.json`, and `.icx/mmu_state.json` via OS advisory locking (`cas_store.lock`), while `/dev/shm/icx-fabric-v1` (or `.icx/icx-fabric-v1.shm`) broadcasts live `⟦Θ:<agent>@<subject>⊢...⟧` frames lock-free in `< 500 ns`.
  - When an implementer agent (e.g., Codex) runs a test suite or modifies a file, the architect/verifier agent (e.g., Claude Code or `agy`) automatically receives the `#icx:<16-hex>` handle in its next turn's Ephemeral Tail Slot and can inspect the exact diff or failure via `icx(op="slice", id="#icx:...", range=[...])` without re-running the test suite.

### 5.3 Short Sessions vs. Marathon Coding / Research Sessions

- **Short Tactical Sessions (`< 30` turns):**
  - Hook-based `PostToolUse` virtualization + the `icx` MCP pager (`deref`, `slice`, `grep`) is sufficient even without running `icx proxy`.
- **Marathon Sessions (`100` to `1,000+` turns / Overnight `/goal` Runs):**
  - Always enable **both** native lifecycle hooks **and** the Cache-Epoch Wire Proxy (`icx proxy` on `127.0.0.1:49380` or `127.0.0.1:8787`), plus `AntigravityAdapter`'s `Stop` continuation hook (`MAX_TOKEN_BUDGET_EXCEEDED` -> `decision: "continue"`).
  - Periodically fold completed milestones via `icx(op="fold")` so `Zone 1` sealed epochs remain compact and provider KV cache hit rates stay `>= 94%`.

### 5.4 Decision Guide: Mode A (Standalone `icx`) vs. Mode B (Dual-Stack `icx` + `ai-memory`)

- **Choose Mode A (Standalone `icx` Only) when:**
  - You want a single, zero-dependency, self-contained Rust binary (`icx`) with zero external daemons or SQLite services.
  - You are running automated CI/CD agents, ephemeral container sandboxes, or strict air-gapped environments where `.icx/` directory persistence (`cas_store.json`, `bitemporal_graph.json`, `mmu_state.json`) provides all required working and cross-session state.
- **Choose Mode B (Dual-Stack `icx` + `ai-memory`) when:**
  - Human engineers and multiple AI harnesses collaborate over weeks/months and benefit from a human-readable Markdown Wiki (`decisions/`, `_rules/`, `gotchas/`), structured cross-session handoff batons (`memory_handoff_*`), and cross-repository agent inboxes (`memory_message_*`) alongside `icx`'s sub-500us machine-native MMU and lossless CAS.

---

## 6. Realized Benefits, Pros/Cons, and Trade-Off Matrix

### 6.1 Quantified Engineering Benefits (`v1.0.0` Verified Benchmarks)

| Metric / Capability | Native Agent Baseline (Unassisted) | With Infinity-Context (`icx` `v1.0.0`) | Underlying Mechanism |
| :--- | :--- | :--- | :--- |
| **Tool Output & Log Token Footprint** | `1.0x` (raw 50 KB–500 KB dumps injected verbatim) | **`8x–25x` reduction** (up to `40x` on test logs) | `StructuralSkeletonizer` + `TitansSurpriseGate` + BPE-aligned `AHTL` (`Profile A` & `Profile B`). |
| **Multi-Turn Prompt Prefix-Cache Hit Rate** | `< 20%` once sliding-window eviction or mid-history pruning starts | **`>= 94.0%`** across 50–200+ turns | 4-Zone `CacheEpochController` + immutable `Zone 1` sealed epochs + `Zone 3` Ephemeral Tail Slot. |
| **Post-Compaction Byte & Line Recoverability** | `0%` (lossy LLM summary destroys exact line numbers & identifiers) | **`100.0%` lossless** (bit-for-bit verified by 256-bit `BLAKE3`) | FastCDC Content-Defined Chunking + `CasStore` + `MerkleContextDag` (`#icx:<16-hex>` / `#icx:<12-hex>`). |
| **Associative & Multi-Hop Graph Recall Latency** | `800 ms – 4,000 ms` (secondary LLM or network vector DB call) | **`< 50 us`** (`~15–35 us` for 2-stage HDC + 201-node CSR PPR) | 10,240-bit `Hypervector` (`[u64; 160]`) + 16-D `FractalCoord` (`FCGA`) + CSR `BiTemporalGraph` PPR. |
| **Cross-Agent Same-Host State Propagation** | Seconds to minutes (or manual copy-paste / `SQLITE_BUSY` contention) | **`< 500 ns`** (`~45 ns` in-memory / `< 500 ns` `/dev/shm` positional I/O) | Lock-free `SeqlockTelemetryBus` (`64 B` `FabricHeader` + `256 x 4,096 B` `SeqlockSlot` ring). |
| **Pre-Storage & Pre-Wire Secret Leakage** | High risk (raw `.env` or debug dumps enter transcript & cloud logs) | **`0.0%` leakage** (`FLAG_IS_REDACTED` provenance bit set) | 4-Tier `SecretRedactor` (prefixes, PEM blocks, URL/env assignments, Shannon entropy `> 4.65 bits/char`). |

### 6.2 Comprehensive Architecture Trade-Off Matrix

| Evaluation Dimension | (a) Native Agent Only (No `icx`, No `ai-memory`) | (b) `ai-memory` Only (Without `icx`) | (c) `icx` Standalone (Mode A) | (d) `icx` + `ai-memory` Dual-Stack (Mode B) |
| :--- | :--- | :--- | :--- | :--- |
| **Setup Complexity** | **Minimal:** Zero external binaries or hooks. | **Low–Moderate:** Requires `ai-memory` daemon (`:49375`), MCP bridge, and `ai-memory-*` skills. | **Low:** Single static-friendly Rust binary (`icx init`, `icx hook`, `icx mcp-serve`, optional `icx proxy`). | **Moderate:** Configures both `icx` and `ai-memory` hooks/MCP servers in ordered arrays. |
| **RAM & Disk Footprint** | Baseline harness only. | `~50 MiB` binary + `memory.sqlite` + Markdown wiki files. | **`~4–12 MiB` RAM** + `1.0 MiB` `/dev/shm` ring + compact `.icx/` JSON/FastCDC store. | Combined (`~1.0 MiB` `/dev/shm` + `.icx/` + `~/.local/share/ai-memory/`). |
| **In-Flight Tool Output Virtualization** | **None:** A single 200 KB `cargo test` log consumes ~50K tokens every turn thereafter. | **None:** `ai-memory` observes tool calls passively but does not rewrite `updatedToolOutput`. | **Full (`< 500 us`):** Replaces bulky outputs `>= threshold` with 40–120 token `#icx:<hex>` stubs. | **Full (`< 500 us`):** Handled by `icx` while `ai-memory` logs high-level session observations. |
| **Prompt Prefix-Cache Impact** | Degrades sharply after first compaction or sliding-window trim. | Neutral (injects at `SessionStart`, does not pin mid-history epochs). | **Optimal (`>= 94%` hit rate):** Pins `Zone 0`/`Zone 1` epochs and isolates recall in `Zone 3`. | **Optimal (`>= 94%` hit rate):** `SessionStart` wiki context lands in `Zone 0`; `icx` manages `Zone 1`–`Zone 3`. |
| **First-Turn vs. Multi-Turn Latency** | Fast on turn 1; slows down as prompt prefill grows to 100K+ tokens. | `+5–20 ms` on `SessionStart`; slow prefill on long sessions due to unstubbed tool outputs. | **`+0.3 ms` hook overhead; 5x–15x faster multi-turn prefill** due to compact AHTL window + cache hits. | **`+5–20 ms` on `SessionStart`; `< 0.5 ms` per turn** with maximum cache reuse. |
| **Cold-Start & Cross-Session Behavior** | Starts blank or relies on static `CLAUDE.md` / `AGENTS.md`. | Strong prose catch-up via `memory_briefing`, `decisions/`, and single-use handoffs. | Restores full `CasStore`, `BiTemporalGraph`, `MerkleContextDag`, and `HdcAssociativeIndex` from `.icx/`. | **Best of Both:** Human-curated ADRs/handoffs from `ai-memory` + byte-exact `#icx:<hex>` CAS & graph state from `icx`. |
| **Failure & Fallback Mode** | Hard context-limit errors or destructive compaction. | If daemon stops, MCP tools return connection errors while hooks no-op. | **Fail-Open Hook + Fail-Closed Storage:** Malformed hook input emits `{}`; corrupted `cas_store.json` aborts writes without data loss. | Independent failure domains: if either system is offline, the other continues operating cleanly. |
| **Primary Cons / Limitations** | Severe token waste, frequent compactions, loss of exact line numbers, zero cross-agent memory. | Does not compress active context windows, does not prevent tool-output bloat, lossy prose summaries. | `AHTL` is a machine dialect (requires `icx decode-ahtl` or `icx deref` for humans to read); lacks prose wiki curation. | Slightly higher initial configuration surface (requires maintaining hook ordering in `settings.json` / `hooks.json`). |

---

## 7. Typical End-to-End Use Cases & Turn-by-Turn Workflows

### 7.1 Use Case 1: Marathon Debugging & Test-Fix-Test Loop (`PASS -> FAIL -> PASS`)

**Scenario:** An engineer uses Claude Code (or OpenAI Codex) to debug a concurrency regression across a 50-turn test-fix-test loop where `cargo test --workspace` emits 64 KB (`1,420` lines) of output per run.

1. **Turn 1 (Baseline Passing Run — `PASS`):**
   - The agent runs `cargo test --workspace` (`exit_code = 0`, 142 passed, 0 failed).
   - `icx hook --agent claude-code --event PostToolUse --dir .icx` ingests the 64 KB output into `.icx/cas_store.json` as `#icx:11a0b2c3d4e5f601`.
   - `InfinityContextMmu` extracts clauses `pass = 142`, `exit_code = 0`, and records functional edges in `BiTemporalGraph`:
     - `("Bash:cargo test --workspace", "status", "PASS", t_valid = 1794880800010, t_invalid = None)`
     - `("Bash:cargo test --workspace", "fail", "0", t_valid = 1794880800010, t_invalid = None)`
2. **Turn 14 (Regression Introduced — `PASS -> FAIL`):**
   - After editing `crates/icx-store/src/graph_ppr.rs`, the agent re-runs `cargo test --workspace` (`exit_code = 101`, 141 passed, 1 failed at `crates/icx-store/src/graph_ppr.rs:142:9`).
   - `PostToolUse` stores the new 64 KB log as `#icx:7a2e91f0ab12cd34` and replaces the 64 KB tool output with a 298-byte AHTL stub:
     ```text
     ⟪ICX:STUB│#icx:7a2e91f0ab12cd34│raw=64120B→298B│lines=1..1420⟫
     ⟦Ψ:Bash:cargo test --workspace⊢✓pass:141⋮!fail:1⋮!panic:crates/icx-store/src/graph_ppr.rs:142:9⋮!exit_code:101↦#icx:7a2e91f0ab12cd34[1380:1420]⋮Ψ:⠞⠧a9f1⟧
     ```
   - In `BiTemporalGraph`, `upsert_edge` stamps `t_invalid = Some(1794880800140)` on the prior `status = PASS` edge and inserts the new active edge `status = FAIL` pointing to `#icx:7a2e91f0ab12cd34`.
3. **Turn 15 (Housekeeping Command Protection):**
   - The agent runs `Bash:echo checking_files` (`exit_code = 0`).
   - Because `InfinityContextMmu::extract_hook_subject` scopes subjects by command (`"Bash:echo checking_files"` vs. `"Bash:cargo test --workspace"`) **and** `InfinityContextMmu` requires `saw_pass == true` with zero anomalies before clearing a `FAIL` status, the active failure on `"Bash:cargo test --workspace"` is **not** falsely cleared.
4. **Turn 16 (Surgical Line Slicing Without Prompt-Cache Pollution):**
   - Instead of re-running `cargo test` or loading all 1,420 lines, the agent follows the slice hint `[1380:1420]` in the AHTL stub and calls the `icx` MCP tool:
     ```json
     {"op": "slice", "id": "#icx:7a2e91f0ab12cd34", "range": [1380, 1420]}
     ```
   - `McpPagerServer` returns only lines `1380..=1420`. Because `InfinityContextMmu::is_icx_pager_tool("mcp__icx__icx")` is `true`, `PostToolUse` lets those 41 lines pass straight to the model without re-stubbing.
5. **Turn 18 (Fix Verified & Loop Folded — `FAIL -> PASS`):**
   - The agent fixes the bug and re-runs `cargo test --workspace` (`exit_code = 0`, `pass = 142`).
   - `BiTemporalGraph` stamps `t_invalid` on `status = FAIL` and promotes `status = PASS` (`#icx:99c4e01234567890`).
   - The agent calls `{"op": "fold", "q": "fixed_csr_ppr_bounds_check"}` to fold the intermediate debug turns into a single `L2` `SubTrajectoryBranch` node.

---

### 7.2 Use Case 2: Multi-Agent Swarm Coordination (`Claude Code` + `Codex` + `agy` Subagents)

**Scenario:** Three agents run concurrently on the same workstation inside `~/Code/OSS_Public-Projects/Infinity-Context`:
- **Agent 1 (`agy` Coordinator):** Designing architecture and spawning subagents.
- **Agent 2 (`Claude Code` Author):** Implementing Rust changes in `crates/icx-proxy`.
- **Agent 3 (`OpenAI Codex` Verifier):** Running test suites and benchmarks.

```mermaid
sequenceDiagram
    participant CC as Claude Code (Author)
    participant SHM as /dev/shm/icx-fabric-v1 (Seqlock Ring)
    participant CAS as .icx/cas_store.json
    participant CX as OpenAI Codex (Verifier)
    participant AGY as Antigravity CLI (Coordinator)

    CC->>CAS: 1. PostToolUse (git diff, 18 KB) -> #icx:4b89e012a3f4c5d6
    CC->>SHM: 2. Publish Seqlock slot seq=42 (<100ns)\n⟦Θ:claude-code@Bash:git diff⊢...↦#icx:4b89e012a3f4c5d6⟧
    CX->>SHM: 3. user-prompt-submit polls seq > cursor (sees seq=42)
    SHM-->>CX: Injects ⟦Θ:claude-code@Bash:git diff...⟧ into Ephemeral Tail Slot
    CX->>CAS: 4. MCP `icx` (op="deref", id="#icx:4b89e012a3f4c5d6")
    CAS-->>CX: Exact 18 KB diff without re-running git diff
    CX->>CAS: 5. Runs cargo test -> #icx:8f01c2d3e4a5b678 (PASS)
    CX->>SHM: 6. Publish Seqlock slot seq=43\n⟦Θ:codex@shell:cargo test⊢✓pass:188⋮✓exit_code:0⟧
    AGY->>SHM: 7. PreInvocation polls seq > cursor (sees seq=42, 43)
    SHM-->>AGY: UI-silent systemMessage (user_markdown="") updates coordinator
```

1. **Zero-Wait Lock-Free Broadcast:** When Claude Code finishes a tool call, `SeqlockTelemetryBus::publish` increments `FabricHeader.write_seq`, flips `slot.seqlock` to odd (`(seq * 2) | 1`), writes the 256-bit HDC sketch, 16-D `fractal_q15_words`, and UTF-8 `ahtl_words`, and releases `slot.seqlock` as even (`(seq + 1) * 2`).
2. **Per-Agent Cursor Isolation:** `InfinityContextMmu` maintains `agent_read_cursors: BTreeMap<u8, u64>` keyed by `AgentKind`. When Codex polls on `user-prompt-submit` and `agy` polls on `PreInvocation`, each agent independently advances its own cursor and receives up to 4 unseen sibling telepathy frames in its Ephemeral Tail Slot.
3. **Subagent Branch Isolation:** When `agy` or Claude Code spawns concurrent subagents (`sub-A` and `sub-B`), `SubagentStart` registers isolated buffers in `mmu.subagent_branches`. Tool calls tagged with `"subagent_id": "sub-A"` never interleave into the parent's `Zone 2` window until `SubagentStop` folds `sub-A` into a single `⟪ICX:SUBAGENT_FOLDED│#icx:<16-hex>│sub-A│<summary>⟫` commit.

---

### 7.3 Use Case 3: Multi-Day / Multi-Session Feature Development with `icx` + `ai-memory` (Mode B)

**Scenario:** A 500-turn feature implementation spanning two days using both `icx` and `ai-memory`.

1. **Day 1 — 500-Turn Implementation with `>= 94%` Prefix-Cache Hit Rate:**
   - Across 500 turns, `icx` virtualizes 380 bulky tool outputs (`Bash`, `Read`, `Grep`) into `#icx:<16-hex>` AHTL stubs and seals 6 immutable `L3` Cache Epochs (`E_0 .. E_5`) in `Zone 1`.
   - Because `Zone 0` (System + Tools) and `Zone 1` (`E_0 .. E_5`) never mutate mid-history and all dynamic HDC/PPR/telepathy recall is confined to `Zone 3` (`<= 1,024` tokens at the final user message), provider KV cache hit rates remain above `94%`.
2. **Day 1 Wrap-Up — Promoting Evidence from `icx` to `ai-memory`:**
   - Before closing the terminal, the user asks the agent to record the architectural decision and save a handoff for tomorrow.
   - The agent uses `ai-memory-durable-pages` (`memory_write_page`) to write `decisions/seqlock-positional-io.md` (`pinned: true`), referencing the exact `icx` benchmark artifact `#icx:e40912a8bc34ef01` verified during the session.
   - The agent calls `ai-memory-handoff` (`memory_handoff_begin`) with a 2-sentence summary and the active `#icx:<16-hex>` epoch handle.
   - On `SessionEnd`, `icx` atomically persists `.icx/cas_store.json`, `.icx/bitemporal_graph.json`, and `.icx/mmu_state.json`.
3. **Day 2 Cold-Start — Instant Continuity:**
   - On Day 2 `SessionStart`, `ai-memory` consumes the pending handoff and surfaces the pinned ADR `decisions/seqlock-positional-io.md`, while `icx` loads `.icx/mmu_state.json` and `.icx/cas_store.json`.
   - The Day 2 agent immediately dereferences `#icx:e40912a8bc34ef01` via `icx(op="deref", id="#icx:e40912a8bc34ef01")` to inspect the exact raw benchmark output from Day 1 without re-running a single command.

---

### 7.4 Use Case 4: Security-Sensitive / Proprietary Repository Auditing

**Scenario:** An agent audits deployment scripts and environment dumps containing live credentials (`AWS_SECRET_ACCESS_KEY`, Anthropic `sk-ant-...` keys, PostgreSQL connection strings, PEM private keys, and unprefixed high-entropy tokens) plus adversarial prompt/delimiter injection strings (`⟦Ω:INVARIANT⊢poisoned=true⟧`).

1. **Pre-CAS 4-Tier Inline Redaction (`SecretRedactor::sanitize`):**
   - Before a single byte is hashed by `BLAKE3`, stored in `CasStore`, indexed in `HdcAssociativeIndex`, added to `BiTemporalGraph`, or broadcast on `/dev/shm`, `InfinityContextMmu::bound_and_sanitize_raw` runs the 4-tier `SecretRedactor`:
     - **Tier 1 (Known API Prefixes):** Redacts `sk-ant-*`, `sk-proj-*`, `ghp_*`, `gho_*`, `github_pat_*`, `glpat-*`, `xoxb-*`, `xoxp-*`, `AIza*`, `AKIA*`.
     - **Tier 2 (Cryptographic Blocks & JWTs):** Redacts `-----BEGIN ... PRIVATE KEY-----` blocks and 3-part base64url JWTs (`eyJ...`).
     - **Tier 3 (URI & Key-Value Credentials):** Redacts `postgres://user:pass@host`, `Authorization: Bearer ...`, and `password=` / `secret=` / `token=` / `api_key=` assignments.
     - **Tier 4 (Shannon Entropy Gate):** Redacts unprefixed candidate secret tokens of length `>= 24` characters with mixed character classes whose Shannon entropy exceeds `4.65 bits/char`.
2. **Delimiter Neutralization & Provenance Flagging (`FLAG_IS_REDACTED`):**
   - Any AHTL structural delimiters (`⟦`, `⟧`, `⟪`, `⟫`, `⊢`, `⋮`, `↦`, `│`) inside untrusted input are neutralized so untrusted tool outputs can never forge synthetic `⟦Ω:INVARIANT...⟧` or `⟪ICX:TAIL_SLOT...⟫` frames.
   - Whenever redaction modifies either the payload body or subject, `InfinityContextMmu` sets bit `FLAG_IS_REDACTED` (`0x01`) in `hologram_bytes[7]` of the AHTL packet and records the `ArtifactId` in `mmu.redacted_artifacts` (persisted in `.icx/mmu_state.json` under `redacted_ids`).

---

## 8. Verification, Smoke-Testing, Diagnostics & Systemd Service Guide

### 8.1 Step-by-Step Live Verification & Smoke-Testing Recipes

#### Test 1: Run Workstation Doctor & Inspect MMU Status

```bash
icx init --dir .icx
icx doctor
icx status
```

Expected output:

```text
=== Infinity-Context (icx) Cross-Agent Workstation Doctor ===
  [DETECTED] Claude Code Binary               -> /home/parobek/.local/bin/claude
  [DETECTED] Claude Settings                  -> /home/parobek/.claude/settings.json
  [DETECTED] OpenAI Codex Binary              -> /home/parobek/.local/bin/codex
  [DETECTED] Codex Hooks Config               -> /home/parobek/.codex/hooks.json
  [DETECTED] Codex State SQLite               -> /home/parobek/.codex/state_5.sqlite
  [DETECTED] Antigravity CLI (agy)            -> /home/parobek/.local/bin/agy
  [DETECTED] Antigravity Hooks                -> /home/parobek/.gemini/config/hooks.json
  [DETECTED] Hermes Agent Source              -> /home/parobek/.hermes/hermes-agent/agent/context_engine.py
  [DETECTED] HTTPS Proxy Transport (curl)     -> /usr/bin/curl
  [DETECTED] POSIX Shared Memory (/dev/shm)   -> /dev/shm
Doctor status: READY
⟪ICX:STATUS│cas_artifacts=0│redacted_artifacts=0│unique_chunks=0│dag_nodes=0│sealed_epochs=0│hdc_entries=0│graph_edges=0│shm_seq=0⟫
```

#### Test 2: Smoke-Test `PostToolUse` Hook Interception & Secret Redaction

Feed a synthetic bulky `PostToolUse` JSON payload (`> 512` bytes) to `icx hook` and verify that it returns `updatedToolOutput` with an `#icx:<16-hex>` handle and panic extraction:

```bash
python3 -c '
import json
payload = {
    "tool_name": "Bash",
    "tool_input": {"command": "cargo test --workspace"},
    "tool_response": ("test case_ok ... ok\n" * 40) + "thread '\''main'\'' panicked at src/lib.rs:99:5: index out of bounds\n",
    "exit_code": 101
}
print(json.dumps(payload))
' | icx hook --agent claude-code --event PostToolUse --dir .icx
```

Expected JSON output on `stdout`:

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PostToolUse",
    "updatedToolOutput": "⟪ICX:STUB│#icx:89a4c10e7b2d3f12│raw=865B→214B│lines=1..41⟫\n⟦Ψ:Bash:cargo test --workspace⊢✓pass:40⋮!panic:src/lib.rs:99:5⋮!exit_code:101↦#icx:89a4c10e7b2d3f12[1:41]⋮Ψ:...⟧"
  }
}
```

#### Test 3: Smoke-Test the Single-Tool Polymorphic MCP Pager (`icx mcp-serve`)

Verify `initialize`, `tools/list`, and `tools/call` (`status`, `recall`, `xagent`, `fold`, `deref`, `slice`, `grep`) over `stdio` JSON-RPC 2.0:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"icx","arguments":{"op":"status"}}}' \
  '{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"icx","arguments":{"op":"fold","q":"begin:smoke_loop"}}}' \
  '{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"icx","arguments":{"op":"fold","q":"smoke_loop_resolved"}}}' \
  '{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"icx","arguments":{"op":"recall","q":"cargo test panic"}}}' \
  '{"jsonrpc":"2.0","id":7,"method":"tools/call","params":{"name":"icx","arguments":{"op":"xagent"}}}' \
  | icx mcp-serve --dir .icx
```

To also test `deref`, `slice`, and `grep` against a workspace file stored in `.icx/`:

```bash
HANDLE=$(icx encode-ahtl Cargo.toml --dir .icx --profile b | grep -oE '#icx:[0-9a-f]{16}' | head -n1)
echo "Encoded handle: $HANDLE"
icx deref "$HANDLE" --lines 1:10 --dir .icx
```

#### Test 4: Smoke-Test the Cache-Epoch Wire Proxy & `proxy-transform`

Verify deterministic middle-turn virtualization, `cache_control: {"type": "ephemeral"}` breakpoint pinning, and `⟪ICX:TAIL_SLOT│...⟫` injection for both Anthropic (`/v1/messages`) and OpenAI (`/v1/responses`):

```bash
python3 -c '
import json
req = {
    "model": "claude-opus-4-6",
    "messages": [
        {"role": "user", "content": "pub fn alpha() -> usize { 1 }\n" * 30},
        {"role": "assistant", "content": "Acknowledged alpha."},
        {"role": "user", "content": "Where is alpha defined?"}
    ]
}
print(json.dumps(req))
' | icx proxy-transform --provider anthropic --dir .icx | python3 -m json.tool
```

Verify in the formatted JSON output that:
1. `messages[0].content` is compressed into an AHTL packet (`⟦Δ:turn_0...⟧`) with a `#icx:<16-hex>` handle stored in `.icx/cas_store.json`.
2. `messages[0].cache_control` is set to `{"type": "ephemeral"}` (exactly 1 epoch breakpoint).
3. `messages[2].content` has `\n⟪ICX:TAIL_SLOT│agent=claude-code│...⟫` appended at the tail.

#### Test 5: Run the Hardware SLO Benchmark Suite

```bash
icx benchmark
```

Expected output:

```text
=== Infinity-Context (icx) SLO & Accuracy Benchmark Suite ===
  [PASS] 10,240-bit HDC Bind (^) + Permute: 14.20 ns / op (1,000 ops)
  [PASS] 10,240-bit HDC Hamming Scan:       12.40 us / 1,000 nodes (124.00 us / 10k nodes, min_dist=4892)
  [PASS] Fractal IFS Suffix Match:          8.10 ns / pair (shared_depth=6)
  [PASS] Two-Stage HDC + CSR PPR:           18.20 us (two-stage query), 14.50 us (201-node CSR PPR)
  [PASS] AHTL Compression Ratio:            78.4x (19694 B -> 251 B in 42.10 us)
  [PASS] FastCDC + BLAKE3 CAS Ingest (64K): 64.50 us (64680 B across 8 chunks)
  [PASS] Atomic Seqlock Broadcast:          44.2 ns / slot (1,000 publishes)
  [PASS] Full Synchronous Hook Round-Trip:  118.40 us (PostToolUse virtualization)
  [PASS] 50-Turn NIH + Cache-Epoch:         100.00% prefix cache hit rate, 100% lossless passkey recovery (4.12 ms)
All Infinity-Context SLO gates PASSED.
```

---

### 8.2 Complete `icx` CLI Reference (`crates/icx-cli`)

| Command & Flags | Purpose & Operational Behavior |
| :--- | :--- |
| `icx init [--dir <PATH>]` | Initializes `<PATH>` (default `.icx`), verifies or creates `cas_store.json` (`mode 0600`), acquires `cas_store.lock`, and initializes the `1,048,640`-byte `icx-fabric-v1.shm` Seqlock ring (`⟪ICX:INIT│dir=.icx│status=READY⟫`). |
| `icx doctor [--dir <PATH>]` | Probes local agent binaries and hook configs (`~/.local/bin/claude`, `~/.claude/settings.json`, `~/.local/bin/codex`, `~/.codex/hooks.json`, `~/.codex/state_5.sqlite`, `~/.local/bin/agy`, `~/.gemini/config/hooks.json`, `~/.hermes/hermes-agent/agent/context_engine.py`, `curl`, `/dev/shm` writability) and verifies workspace `CasStore` + `MerkleContextDag` cryptographic integrity (`[VERIFIED] Workspace Storage & Merkle-DAG`). |
| `icx status [--dir <PATH>]` | Prints a single-line AHTL telemetry frame: `⟪ICX:STATUS│cas_artifacts=<N>│redacted_artifacts=<N>│unique_chunks=<N>│dag_nodes=<N>│sealed_epochs=<N>│hdc_entries=<N>│graph_edges=<N>│shm_seq=<N>⟫`. |
| `icx benchmark` | Runs the 9-stage hardware SLO verification suite (`10,240-bit HDC Bind + Permute`, `10,240-bit HDC Hamming Scan`, `Fractal IFS Suffix Match`, `Two-Stage HDC + CSR PPR`, `AHTL Compression Ratio`, `FastCDC + BLAKE3 CAS Ingest (64K)`, `Atomic Seqlock Broadcast`, `Full Synchronous Hook Round-Trip`, and `50-Turn NIH + Cache-Epoch`). |
| `icx deref <HANDLE> [--lines <START:END>] [--dir <PATH>]` | Losslessly reconstructs the full UTF-8 byte stream or an inclusive 1-based line slice (`START:END`, e.g., `1:50`) for any 64-bit (`#icx:<16-hex>`) or 48-bit (`#icx:<12-hex>`) handle from `<PATH>`. |
| `icx encode-ahtl <FILE> [--dir <PATH>] [--profile <a\|b>]` | Redacts secrets, ingests `<FILE>` into `<PATH>` (default `.icx`), updates the HDC/Fractal/Bi-Temporal indices, and prints either a **Profile B** dual-channel packet (`⟦Δ:...⟧`, default `--profile b`) or a **Profile A** 6-section canonical wire packet (`⟦AHTL v1│#icx:<12-hex>│fc:<64-hex>│hv:<16-hex>│<64-braille>│...⟧`, `--profile a`). |
| `icx decode-ahtl '<AHTL_PACKET>'` | Auto-detects Profile A or Profile B (`WirePacket::parse_auto`), validates checksums/headers, and prints the structured breakdown (Kind, Subject, Line Range, Deref ID, Slice Hint, Fractal FC, HDC Digest, 64-band Braille spectrum, and clauses). |
| `icx hook --agent <claude-code\|codex\|agy\|hermes> --event <EVENT> [--dir <PATH>]` | Executes a synchronous `< 500 us` lifecycle hook, reading the harness's JSON payload from `stdin` and emitting the schema-validated response JSON on `stdout` (failing safe to `{}` on malformed input). |
| `icx mcp-serve [--dir <PATH>]` | Runs the Single-Tool Polymorphic MCP Virtual Pager (`icx`) over `stdio` (JSON-RPC 2.0 `initialize`, `notifications/initialized`, `ping`, `tools/list`, `tools/call`), reloading and persisting `<PATH>` (or `ICX_DIR`, default `.icx`) transactionally on every operation. |
| `icx proxy-transform [--provider <anthropic\|openai\|gemini>] [--dir <PATH>]` | Reads an outbound Anthropic (`/v1/messages`), OpenAI (`/v1/chat/completions`, `/v1/responses`), or Gemini (`/v1beta/models/*`) JSON request from `stdin`, applies `WireProxyTransformer` transactionally against `<PATH>` (default `.icx`), and writes the Cache-Epoch-pinned JSON body to `stdout`. |
| `icx proxy [--listen <ADDR:PORT>] [--provider <anthropic\|openai\|gemini>] [--upstream <URL>] [--dir <PATH>] [--max-requests <N>]` | Runs the resident HTTP/1.1 & SSE Cache-Epoch Reverse Proxy listening on `127.0.0.1:49380` (or `ICX_PROXY_BIND`), forwarding transformed requests to `--upstream` / `ICX_UPSTREAM_URL` (or the provider's default upstream) and expanding inline `⟪!icx:deref(...)⟫` directives on SSE/JSON responses. |

---

### 8.3 Systemd User Service Templates (`systemd --user`)

Install `~/.config/systemd/user/icx-proxy.service` to keep the Cache-Epoch Wire Proxy and `/dev/shm` fabric resident across login sessions:

```ini
[Unit]
Description=Infinity-Context (icx v1.0.0) Cache-Epoch Wire Proxy & Virtual Context MMU
Documentation=https://github.com/doublegate/Infinity-Context/blob/main/docs/deployment-and-agent-integration-guide.md
After=network-online.target

[Service]
Type=simple
WorkingDirectory=%h/Code/your-workspace
ExecStartPre=/usr/bin/env icx init --dir .icx
ExecStart=/usr/bin/env icx proxy --listen 127.0.0.1:49380 --dir .icx
Restart=on-failure
RestartSec=2s
Environment="RUST_LOG=info"
Environment="ICX_DIR=.icx"
Environment="ICX_PROXY_BIND=127.0.0.1:49380"
Environment="ICX_SHM_PATH=/dev/shm/icx-fabric-v1"
NoNewPrivileges=true
PrivateTmp=false

[Install]
WantedBy=default.target
```

> **Note:** Replace `%h/Code/your-workspace` with your target repository path (or use a parameterized template unit `icx-proxy@.service` with `WorkingDirectory=%h/Code/%i`). Ensure `icx proxy`, `icx hook`, and `icx mcp-serve` operating on the same repository point to the same `--dir` / `ICX_DIR` (e.g., `.icx` inside `WorkingDirectory`) so `#icx:<16-hex>` CAS handles and `workspace_hash` `/dev/shm` filters match across all three surfaces.

Enable and verify the service:

```bash
systemctl --user daemon-reload
systemctl --user enable --now icx-proxy.service
systemctl --user status icx-proxy.service
```

---

### 8.4 Troubleshooting Guide

| Symptom / Error Message | Root Cause | Resolution |
| :--- | :--- | :--- |
| **`Invalid Seqlock magic` or stale `/dev/shm/icx-fabric-v1` file** | A previous pre-v1.0.0 build or truncated file left a non-`1,048,640`-byte segment in `/dev/shm` or `.icx/icx-fabric-v1.shm`, or file ownership differs from `$UID`. | `SeqlockTelemetryBus::open_shared_file` rejects existing files with invalid magic (`ICX_SHM_MAGIC = 0x4943_585F_4641_4231`) or mismatched size. Remove the stale file (`rm -f /dev/shm/icx-fabric-v1 .icx/icx-fabric-v1.shm`) and run `icx init --dir .icx`. |
| **`ERR: ambiguous handle prefix #icx:<12-hex>` (`IcxError::AmbiguousHandlePrefix`)** | A 48-bit Profile A short handle (`#icx:<12-hex>`) matched two distinct 64-bit `ArtifactId` entries across `CasStore` and `MerkleContextDag`. | Pass the full 64-bit 16-hex handle (`#icx:<16-hex>`) shown in the `⟪ICX:STUB│...⟫` header or run `icx(op="recall", q="<subject>")` to list the full 16-hex `ArtifactId`. |
| **Redacted placeholder (`[REDACTED_SECRET]` / `FLAG_IS_REDACTED`) appears in `icx deref` output** | `SecretRedactor` detected a credential, PEM block, JWT, or high-entropy token (`> 4.65 bits/char`) during ingestion and scrubbed it *before* writing to `CasStore`. | This is intentional fail-closed security behavior (`mmu.is_artifact_redacted(id) == true` and `hologram_bytes[7] & 0x01 != 0`). `icx` never stores raw secrets in `.icx/cas_store.json`. Inspect the original local file directly if needed. |
| **`cas_store.json` parse or `BLAKE3` verification error on `icx encode-ahtl` / `icx init`** | Disk corruption or manual editing modified `.icx/cas_store.json`. | `icx` intentionally **fails closed** and refuses to overwrite a corrupted `cas_store.json` with an empty store. Back up `.icx/cas_store.json`, inspect or restore from backup, and re-run `icx init --dir .icx`. |
| **`timed out acquiring workspace lock .icx/cas_store.lock`** | Another process is holding an active OS advisory `flock` on `.icx/cas_store.lock` for `> 5,000 ms` (note: crashed processes automatically release OS file locks on exit even if `cas_store.lock` remains on disk). | Check `lsof .icx/cas_store.lock` for any hung process; do not delete `cas_store.lock` while an active writer is running. |
