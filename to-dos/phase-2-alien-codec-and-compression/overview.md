# Phase 2 — Alien Holographic Token Lattice (`AHTL`) & Structural Skeletonizers

## Goal

Implement the non-human BPE-aligned **Alien Holographic Token Lattice (`AHTL`)** codec (`crates/icx-codec`), Unicode Braille-256 holographic sketch encoding, domain-specific AST/log/diff structural skeletonizers, and the pre-ingestion secret redaction and delimiter-escaping security gate.

## Exit Criteria

- [x] `AhtlPacket` renders and parses round-trip with 100% fidelity.
- [x] Braille-256 (`U+2800..U+28FF`) maps 64-bit topological sketches bijectively in 8 characters.
- [x] `StructuralSkeletonizer` compresses verbose compiler/test logs and source files by `12x–40x` while preserving 100% of error lines, panic locations, and function signatures.
- [x] `SecretRedactor` strips all API keys, PEM blocks, and `.env` secrets prior to CAS or `/dev/shm` ingestion.

## Sprints

- [Sprint 1 — AHTL Grammar, Parser & Braille-256 Bijection](sprint-1-ahtl-grammar-and-braille.md)
- [Sprint 2 — AST, Test-Log & Diff Structural Skeletonizers](sprint-2-structural-skeletonizers.md)
- [Sprint 3 — Pre-Ingestion Secret Redactor & Delimiter Escaping](sprint-3-secret-redaction-and-safety.md)

## Reference Docs

- [docs/alien-holographic-codec.md](../../docs/alien-holographic-codec.md)
- [docs/security-and-threat-model.md](../../docs/security-and-threat-model.md)
