# Local Linux candidate package qualification — complete

Local candidate package checks are **complete for all four advertised Linux variants**, and the accepted final production source hashes match the frozen package build snapshot. This is **not committed/tagged release certification**. No tag, commit, push, installation, user configuration change or icx daemon invocation was performed. Package smoke writes were confined to task-owned HOME/XDG/project directories under `/tmp/Infinity-Context/v120`; container runs used no network and no credentials/configuration mounts. The final redactor campaign was accepted: [attempt 7](fuzz-redact-attempt7.log) completed 859,078 runs in 901 seconds with normal exit 0; the [seeded HDC campaign](fuzz-hdc-final-seeded-summary.json) completed 2,231,269 runs in 901 seconds with exit 0. [Local acceptance](package-local-acceptance.json) binds these results to the matching production hashes. Exact-commit CI, governance, quiet performance qualification, release attestation and publication remain separate release decisions/operations.

## Completed local candidate checks

| Advertised label | Build environment / CPU | Linkage gate | Isolated host + stripped package smoke | Ubuntu 22.04 / glibc 2.35 smoke | Deterministic archive |
| --- | --- | --- | --- | --- | --- |
| `x86_64-unknown-linux-musl` | host, Rust default self-contained linker; `x86-64` | PASS: static PIE; no PT_INTERP or DT_NEEDED | PASS: doctor/init/status | PASS: doctor/init/status | PASS: repeated bytes identical |
| `x86_64_v3-unknown-linux-musl` | host, Rust default self-contained linker; `x86-64-v3` | PASS: static PIE; no PT_INTERP or DT_NEEDED | PASS: doctor/init/status | PASS: doctor/init/status | PASS: repeated bytes identical |
| `x86_64-unknown-linux-gnu` | offline cached Bookworm, mounted Rust 1.100.0; `x86-64` | PASS: highest GLIBC symbol 2.34 ≤2.35 | PASS: doctor/init/status | PASS: doctor/init/status | PASS: repeated bytes identical |
| `x86_64_v3-unknown-linux-gnu` | offline cached Bookworm, mounted Rust 1.100.0; `x86-64-v3` | PASS: highest GLIBC symbol 2.34 ≤2.35 | PASS: doctor/init/status | PASS: doctor/init/status | PASS: repeated bytes identical |

[Full local results](package-four-variant-results.json) retain binary and archive hashes, compilation CPU flags, linkage output, every smoke command/exit/output, stripped-package smoke, archive member lists and all packaged input hashes. [Ubuntu runtime results](package-ubuntu2204-smoke-results.json) independently record all twelve command exits against the **stripped packaged executables**. The runtime image is pinned as `ubuntu@sha256:f64b2ffc2f10ab8f8505aa56b2664bd846cbb9df87c7e2532e30f7667021c0e6`, and `ldd --version` confirms glibc 2.35. The same image has no `/lib/ld-musl-x86_64.so.1`; successful musl smoke therefore exercises standalone static deployment without a musl runtime installation. The test host supports the required v3 ISA features; baseline execution on an older physical CPU was not tested.

GNU variants were **built in Bookworm/glibc 2.36**, not Ubuntu 22.04, then inspected for their actual required symbol ceiling and executed under glibc 2.35. This distinguishes the build environment from the demonstrated symbol requirement/runtime environment. The release workflow still builds GNU on Ubuntu 22.04; no change to its build flags was necessary. [Offline container build commands/results](package-bookworm-build-results.json) retain the exact invocation. The cached build image repository digest is `rust@sha256:ade39a710ef85f3506cffcfec3e8a24ba090badef1dff2fb798f71d878833fea`; the mounted Rust toolchain is exactly 1.100.0, not whichever compiler ships inside that image.

## Retained negative evidence and musl diagnosis

[Original binary inspection/smoke](package-preliminary-results.json) records four ineligible earlier candidates. Both old musl executables had a PT_INTERP musl loader path and crashed with SIGSEGV for doctor/init/status; both old host GNU binaries required GLIBC 2.39, above the advertised 2.35 ceiling. The host-native GNU build also used `target-cpu=native`, which is not evidence of an exact x86-64-v3 build. Later successful rebuilds do not erase those outcomes.

[Standalone linkage probes](package-linkage-probes.json) isolate the local musl linker issue: this host's explicit Rust `linker=musl-gcc` introduces a PT_INTERP entry through its GCC specs, even with `+crt-static` or `-static-pie`. The Rust musl target already enables crt-static by default. Omitting the forced Rust linker gives static PIE; explicitly adding `-C link-arg=-static` to musl-gcc gives static EXEC. The full successful rebuilds use the former approach. `CC_x86_64_unknown_linux_musl=musl-gcc` is retained **only for bundled SQLite C compilation**, matching the release workflow. [Default-linker full-build outcomes](package-default-musl-build-results.json) retain both actual icx build exits; tiny probes alone were never counted as icx qualification.

Initial Docker cache access without sandbox escalation was denied; authorized local escalation succeeded. Initial Podman configuration attempted a write under read-only `/run/user/1000/libpod` and was not used. No user daemon/configuration was repaired or changed. [Environment evidence](package-environment.json) records the cache/tool availability. After the cached Bookworm builds passed, the official Ubuntu 22.04 runtime image was fetched solely for the separately authorized glibc 2.35 smoke. Builds and all actual runtime tests remained offline.

## Archive layout, notices and reproducibility boundary

The [local qualification script](qualify-local-packages.py) mirrors the workflow layout: stripped executable, `LICENSE`, `COPYING`, `README.md`, `CHANGELOG.md`, `THIRD-PARTY-NOTICES.txt`, GPL/AGPL full texts under `LICENSES/`, plus `MUSL-COPYRIGHT.txt` for musl. [Notices provenance](package-notices-provenance.json) records a successful `cargo about generate --locked --offline --fail` with the workflow config/template and input/output hashes. The local Arch musl notice comes from `/usr/share/licenses/musl/COPYRIGHT`; the Ubuntu workflow's `/usr/share/doc/musl/copyright` is a different distribution path. No licensing/governance text was changed.

Archives normalize modes, owner/group IDs to 0, ordering and mtime to the recorded commit epoch, then use `gzip -n`. Each staged input set was packaged twice with byte-identical output. This proves deterministic **packaging of fixed inputs**, not bit-reproducible compilation across machines/toolchains. [Fixture mechanics](package-layout-fixture-result.json) separately verified layout, uid/gid, permissions and timestamps before packaging the icx candidates; that fixture used a linkage probe and was not an additional release binary.

[Collected dist checksums](package-dist-checksums.json) records all four local archives and a passing `sha256sum -c SHA256SUMS`. They remain in a task-owned preliminary directory, not attached to any release. Archive SHA256:

| Label | SHA256 |
| --- | --- |
| `x86_64-unknown-linux-musl` | `a5ac57be57c9c31905650a77a244a9a562a8b38dafcc79f371ac76318d7f014f` |
| `x86_64_v3-unknown-linux-musl` | `518b579a6830b975ab01b9885e28ca89d5c86c3a6f61696dfc57b8070a5959bc` |
| `x86_64-unknown-linux-gnu` | `53289ff970179d2c55caa8d0c12741099b4660621aa84c2a29fc625553147159` |
| `x86_64_v3-unknown-linux-gnu` | `feae28dbb96ae6168ffa457db86e5106f2b0a08172a9355e72d92e64da0f3f09` |

## Source binding and reproduction

[The frozen source manifest](package-rebuild-source-manifest.json) hashes the copied Cargo manifests/lock and crate files, including uncommitted changes. [The drift comparison](package-source-drift-check.json) records subsequent differences. All 43 production/build-input file hashes match the accepted current source, with no new production files. Only test paths differ from the frozen snapshot: `crates/icx-core/tests/integrity_v120.rs`, `crates/icx-core/tests/window_budget_v120.rs`. Repository configuration changes outside the binary input set, including the separate gitleaks exception review, are not certified by this package check. This is a working-tree snapshot, not an exact committed/tagged release. If production source changes, rebuild all four and re-run qualification before reusing these hashes.

The scripts and candidate manifests are reviewable evidence. They do not install anything or publish assets:

```sh
python3 docs/releases/v1.2.0/evidence/qualify-local-packages.py \
  --candidates docs/releases/v1.2.0/evidence/package-four-variant-candidates.json \
  --notices /tmp/Infinity-Context/v120/package-notices-current.txt \
  --report /tmp/Infinity-Context/v120/package-recheck.json
python3 docs/releases/v1.2.0/evidence/smoke-ubuntu2204-packages.py
```

The second script requires authorized access to the local Docker socket and the pinned cached runtime image (`--pull=never`). Build scripts consume the retained frozen source under `/tmp`; those paths/caches are local artifacts and need explicit recreation on another machine. No broad distribution, installation, native-agent integration, performance or model-effectiveness guarantee follows from these startup/workspace smoke tests. Local candidate package qualification is complete against the accepted final production source. Exact committed/tagged release certification remains false pending CI, governance and performance qualification; these local results do not authorize tagging or publication.
