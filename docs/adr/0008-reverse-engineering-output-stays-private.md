# ADR 0008: Reverse-engineering output stays private

Status: accepted, 2026-11-22 UTC. Set by the owner during v1.4.0 Phase 6 Sprint 1 (issue #40). It
amends the rule that the `ref-docs/` corpus is never edited, for one file.

## Context

icx integrates with several agents: Claude Code, OpenAI Codex CLI, the Antigravity CLI (`agy`),
Muse Code and Hermes Agent. To find integration points that the documented hooks do not expose,
Phase 6 Sprint 1 inspects the owner's own installed builds.

Codex (Apache-2.0) and Hermes (MIT) are open source. The methods used for each agent were set by
the owner and are recorded privately.

This kind of analysis produces decompiled and disassembled code, evidence bundles, analysis databases,
string dumps and maps of internal structure. Publishing that material would expose the vendors'
code and internals, with no benefit to icx users.

One file is already public: `ref-docs/agent-internals/rea-binary-and-sqlite-forensics.md`, from
2026-11-16 and published in v1.1.0. It carries binary-level detail about proprietary builds.

## Decision

1. **Raw output never enters the repository.** That covers decompiled or disassembled code, REA
   evidence bundles and snapshots, Ghidra, IDA, Binary Ninja, Hopper and rizin projects, extracted
   bundles and string dumps. `.gitignore` excludes those artifact types, and
   `ref-docs/agent-internals/private/`, wherever they appear.
2. **Detailed per-agent findings live in `ref-docs/agent-internals/private/`,** on disk only.
3. **Public documents state interoperability conclusions only:**
   - which documented or observable surface icx uses;
   - which fields an agent accepts or rejects;
   - which candidate integrations were chosen or rejected, and why.

   They contain no code excerpts, addresses, offsets, internal symbol maps or database schemas of
   proprietary builds. Open-source agents may be cited by file and line in their public source.
4. **The 2026-11-16 ledger** moves to `private/`. The tracked file of the same name becomes a public
   summary, so existing links still resolve. This is the one exception to the corpus's never-edit
   rule. Git history is not rewritten.

## Consequences

**Benefits:**

- Vendor internals are not republished.
- The public record still explains every integration decision.
- Links stay intact.

**Costs:**

- The detailed evidence for decisions about proprietary agents exists only on the owner's
  machine, so it must be backed up outside the repository.
- Earlier revisions of the 2026-11-16 ledger remain in git history and in release archives up
  to v1.3.2.

**Rejected alternatives:**

- *Rewriting git history* to purge the old ledger. It would break clones and tag references, and
  could not recall copies already fetched.
- *Keeping everything public.* It would expose proprietary internals.
