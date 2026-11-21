# Rejected leads and experiment procedures

## Adjudication before acceptance

| Lead | Counterevidence / disposition |
| --- | --- |
| HTTPS client cancellation leaves continuously streaming curl alive indefinitely (reserved 034) | Rejected. Root observed that scoped `BufReader<ChildStdout>` drops before wait, closing curl stdout's read end. The real-curl local SSE countertest exits 23 after 16.093ms. Missing explicit kill/deadline is only a remaining hardening suggestion; no confirmed 034 entry |
| Corrupt CAS line metadata bypasses seals or changes slices | Rejected for the normal checked load: `from_parts` reconstructs/re-hashes content and recomputes line metadata. Unsealed DAG semantics remain a separately qualified limit |
| Every same-user file replacement is a cross-user trust bypass | Rejected. Same-uid processes can obtain install keys and are outside the stated workspace boundary. No other-user race through a private ancestor was demonstrated |
| Profile A packet forgery is already a live hook exploit | Reachability not established. Confirmed only as a public-library framing defect 002; normal skeletonization has prior escaping controls |
| NaN snapshot is an unauthenticated workspace takeover | Rejected escalation. Public decoder accepts NaN 003; normal sealed workspace modification still requires passing the store MAC |
| Future bus sequence corruption proves a naturally observed lapping race | Rejected inference. Probe manipulates public atomic layout deliberately. It proves missing expected-sequence check 032; live race impact stays qualified |
| Legacy MCP version fallback is itself invalid | Rejected. Supported fallback can be negotiated by the client. Missing version/invalid ID/arguments are the separately reproduced 031 defect |
| All MCP revisions require legacy initialize | Rejected. Modern 2026-07-28 discovery/per-request versioning must be evaluated separately |
| `permute(0)` must be identity | Not a defect against the documented implemented family, which explicitly rotates words by 1 at shift 0. Algebraic Π^k notation must not silently impose another contract |
| PPR dangling-node difference necessarily breaks normal CSR graph | Not established. Normal snapshot includes reverse edges and positive weights; the hypothesized dangling condition was not reached. Independent small-vector oracle agrees with production |
| Uppercase field hex acceptance / invalid-hint omission is a hidden codec bug | Rejected where those normalization/omission semantics are documented. Separate normative `fc` scale disagreement 005 remains |
| Local Markdown link checker found a broken tesseract link | False positive: balanced parentheses in filename defeat the simple regex. Target exists. Scan output is preserved as a lead, not a defect |
| Seven findings from installed gitleaks 8.24.3 prove leaked credentials | Rejected. They point to historical synthetic fixtures; exact pinned CI 8.30.1 scanner is separately verified and passes. No credential validity probe or rotation was attempted |
| Every passing line-coverage gate proves strong assertions | Rejected inference. Surviving mutations 073 and YAML/recall counterexamples demonstrate why execution coverage is not semantic correctness |

## Reproduction entry points

Run commands from the repository root; all mutable workspaces and child processes stay under
`/tmp/Infinity-Context/`. Do not point these procedures at live user state.

- [Codec/retrieval procedure](evidence/codec-procedure.md): packet forgery, redactor, NaN, quantization, full 27 dataset matrix and oracle vectors.
- [Real CLI secret recheck](evidence/recheck-yaml-cli.py): invoke with path to an audit-built `icx`; separate config/state/runtime roots and exact CAS dereference.
- [Retrieval repeat](evidence/recheck-retrieval.py): independently reruns varied 10k seed 1; output in `retrieval-recheck.json` agrees with original 33%@5 result.
- [Epoch growth](evidence/run-epoch-growth.py): builds a standalone core public-API probe and records default-controller growth.
- [State procedure](evidence/state-procedure.md): exact bytes, MCP envelopes, coherent corruption, truncation, separate process bus, daemon SIGKILL and failed final flush. [Root repeat](evidence/root-state-recheck.json) independently confirms ACK/loss and false successful exit.
- [Real curl countertest](evidence/state-curl-cancel.py): isolated local SSE, closes read pipe, waits/reaps child; rejected 034 result in accompanying JSON.
- [Validation runner](evidence/validation-run.py), [benchmark runner](evidence/validation-benchmark.py), [fuzz runner](evidence/validation-fuzz.py), [mutation runner](evidence/validation-mutation.py): inspect command/execution ledgers as well as runner source. Some host reruns and expanded mutation partitions use separately recorded commands.
- [Inventory](evidence/inventory.py), [claim/link leads](evidence/document-check.py), [Unicode oracle](evidence/check-braille-table.py), [consolidation](evidence/consolidate.py): deterministic evidence and cross-reference checks.

Runner scripts may rely on cached tool versions, the audited dependency constraints, and Linux
Unix-socket semantics. The evidence records their actual commands and exits. A reproduction
failure on a different cache/platform must be diagnosed rather than silently counted as a
finding reproduction or passing gate.
