# Upgrading an existing workspace to the v1.2.0 candidate

This is a development candidate until its release record says qualification is complete.

1. Stop the workspace's resident daemon through your process supervisor and wait for exit.
   Preserve a private backup of the workspace directory, including SQLite sidecar files, and
   the installation's private configuration/seal key. Keep backups out of source archives.
2. Install the matching Linux baseline or x86-64-v3 binary. The v3 build requires AVX2-class
   hardware. Musl builds are static; GNU builds retain their checked glibc minimum requirement.
3. Run `icx doctor`, then use the existing workspace normally. A repository-local legacy store
   is still ignored unless you explicitly adopt it with `icx trust --dir PATH`.
4. Inspect the model policy with `icx context-budget --dir PATH`. For example:

   ```sh
   icx context-budget --dir PATH --context-tokens 32768 --completion-reserve 4096 --external-reserve 4096
   ```

   Reserve the actual external system/user input, schemas and completion space for your model.
   The identified estimator gives icx-managed UTF-8 bytes one token of allowance each; it is not
   a measured provider tokenizer. Increasing context limits is not a guarantee that your host
   or provider accepts the resulting full request. Changing policy can invalidate cache prefixes.
5. Verify a known old handle with `icx deref HANDLE --dir PATH --raw`. It must return the same
   post-redaction bytes. Redaction fixes affect new ingestion; old stored secrets are not
   automatically rewritten, purged or rotated.

Version 03 HDC snapshots are read and rebuilt from preserved CAS events. Saves write 04 with
exact lexical postings. Existing CAS identities and Q15 Profile A coordinate interpretation
are preserved. An older binary cannot read 04: restore the pre-upgrade backup for downgrade.
Do not delete source artifacts or edit snapshot magic bytes to force a downgrade.

If an older binary does meet an upgraded workspace it fails closed: `icx status` and the hooks
print `failed to load workspace state ... snapshot magic/version mismatch` on stderr, hooks pass
the agent's input through unchanged, and nothing is written, so no stored data is lost (verified
against a copied v1.1.0 workspace of 202 artifacts). icx is simply inactive for that project
until a v1.2.0 binary is back. All agents share one `icx` binary, so upgrade it once for every
agent rather than mixing versions on the same workspace.

Daemon success now follows a committed SQLite WAL FULL save, so real hook latency includes
persistence. Save failures return errors. Native Codex hooks capture output and provide supported
start/prompt recall; they do not replace original PostToolUse output. MCP accepts the advertised
canonical tool name `icx` with explicit typed operations; invalid legacy aliases now fail.

## Moving on to v1.2.1

v1.2.1 writes `ICXHDC05` and rebuilds the token postings of a v1.2.0 (`ICXHDC04`) index from the
CAS event log on first load; no data changes. As with v1.2.0, upgrade the one `icx` binary all
agents use, and keep a pre-upgrade backup if a downgrade to v1.2.0 might be needed.
