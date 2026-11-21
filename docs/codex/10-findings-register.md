# Consolidated findings register

Revision `a996d59f1eb468e7c386747e15f4c7e58d08410e`. Stable IDs have reserved gaps. Severity applies within the stated category: governance questions and unsupported claims are not confirmed security exploits. 19 entries; severity counts: high=1, low=8, medium=10.

Full prerequisites, impact, reproduction/source proof, counterevidence, recommendations and acceptance checks are in the linked reports. [CSV companion](findings.csv).

| ID | Severity | Category | Finding and full evidence |
| --- | --- | --- | --- |
| ICX-CODEX-001 | Medium | security | [YAML scalar credentials survive ingestion redaction](01-ahtl-conformance.md) |
| ICX-CODEX-002 | Low | codec integrity | [Profile A public skeleton field injects extra stream packets](01-ahtl-conformance.md) |
| ICX-CODEX-003 | Low | robustness | [HDC snapshot decoder accepts nonfinite cached state](02-mathematics-and-retrieval.md) |
| ICX-CODEX-004 | High | retrieval correctness | [Recall remediation fails on varied-content corpus growth](02-mathematics-and-retrieval.md) |
| ICX-CODEX-005 | Medium | specification interoperability | [Fractal wire codec disagrees with normative Q1.14 scale](01-ahtl-conformance.md) |
| ICX-CODEX-006 | Low | correctness output stability | [Redaction changes again when redacted payload is processed](01-ahtl-conformance.md) |
| ICX-CODEX-030 | Medium | durability and failure reporting | [Acknowledged daemon artifacts can disappear before persistence](03-integrity-persistence-security.md) |
| ICX-CODEX-031 | Low | protocol correctness | [MCP accepts invalid envelopes and arguments as successful operations](05-integrations-proxy.md) |
| ICX-CODEX-032 | Low | concurrency and specification discrepancy | [Bus reader accepts coherent slots outside its expected sequence](04-bus-concurrency.md) |
| ICX-CODEX-033 | Low | public API robustness | [Extreme public chunker configuration panics](03-integrity-persistence-security.md) |
| ICX-CODEX-035 | Medium | integration capability gap | [Codex adapter emits an explicitly unsupported replacement field](05-integrations-proxy.md) |
| ICX-CODEX-070 | Medium | release control defect | [Release ancestry gate does not require successful CI](07-supply-chain-governance.md) |
| ICX-CODEX-071 | Medium | governance evidence defect | [Unicode PDF and ISO archive licensing remain unsupported by Unicode-3.0 annotation](07-supply-chain-governance.md) |
| ICX-CODEX-072 | Low | measurement limitation | [Benchmark presents extrapolated 10k-node time beside measured 1k-node time](06-validation-performance.md) |
| ICX-CODEX-073 | Medium | test assurance gap | [Independent mutation samples still expose unpinned production behaviors](06-validation-performance.md) |
| ICX-CODEX-074 | Medium | governance question | [Patent and commercial text still requires counsel reconciliation with distributed GPL terms](07-supply-chain-governance.md) |
| ICX-CODEX-100 | Medium | documentation contradiction | [Accepted ADRs and completed roadmap retain superseded guarantees](08-documentation-and-claim-traceability.md) |
| ICX-CODEX-101 | Medium | architecture limitation and unsupported capability | [Production epoch accumulation has no automatic overall context cap](09-context-budget-and-enhancements.md) |
| ICX-CODEX-102 | Low | reference specification defect | [Worked Braille example contains a different Unicode character](08-documentation-and-claim-traceability.md) |
