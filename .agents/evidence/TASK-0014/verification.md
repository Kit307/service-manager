# Service operations verification

## Implemented paths
The existing shared Manager owns configured-port preflight, dependency readiness, sequence cancellation, captured-run history and verified loopback URL checks. Desktop adds confirmed Kill; MCP exposes four read-only evidence tools without a kill mutation.

## Automated proof
- Final `cargo test --no-default-features`: 51 passed, 1 opt-in Docker test ignored, including deterministic cancellation during preflight.
- Final integrated frontend suite: 81 passed in 13 files with `--maxWorkers=1`, including the multi-pane workspace. Earlier concurrent runs under native compilation had fixture-loading timeouts; unchanged focused and serial runs passed without increasing timeouts.
- ESLint, TypeScript/production frontend build and native-target Clippy with warnings denied passed. Rust formatting passed.
- Real disposable process/HTTP fixtures verified external TERM, protected self/stale identity refusal, managed Stop, Ready ordering, restart, failure before dependent launch, cancellation, URL reachability and per-run history.
- Real MCP SDK/proxy verified all 16 advertised tools, read-only annotations on the new four, exact historical-run retrieval after restart, bad IDs/run IDs and revoked-token rejection. No new read call started a run.
- Private history tests cover persistence, bounds, redaction, corruption/symlink/oversize rejection, transactional failure, interrupted records and old-run completion interleaving with a replacement run.

## Review and resolved races
Independent source review identified a fallible URL lookup and managed Kill run race. URL lookup now returns a controlled error; confirmed Stop matches runId and process-group leader and transitions under the same store lock. Regression proves a stale confirmed run cannot stop its replacement. A deterministic preflight-cancel regression proves cancellation while waiting for config prevents any run/history creation.

## Limits
All runtime fixtures were disposable. No user service/config/token or installed app was restarted/replaced. Live Docker and external model sessions were not repeated. External Kill TERM and managed Stop were exercised; forced external SIGKILL escalation was not forced. Redaction is best effort for unknown application-specific secrets. Port preflight does not reserve a port against other processes.
