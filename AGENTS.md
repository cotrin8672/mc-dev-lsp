# Execution cost and verification

- Prefix shell commands with `rtk`.
- Keep full build/test logs on disk. Use compact runners and read their final summaries. Read only the relevant bounded failure excerpt when needed to fix a failure.
- Do not poll logs or repeatedly read unchanged code to obtain progress. Wait for subagent completion reports.
- Run E2E through the compact runner with a finite timeout. On timeout, stop only its owned process tree and report failure; never keep waiting on the same hung run.
- Log economy does not prohibit hang detection: track process IDs and deadlines, and inspect one bounded failure excerpt when a run exceeds its deadline.
- Assign bounded tasks with complete requirements. Avoid duplicate investigations and incremental scope additions. When using subagents, use `gpt-5.6-luna` with `max` reasoning as requested by the user.
- Run affected tests after a change; run the full suite once when changes are stable. Repeat checks only for new changes, failures, or a specific unresolved concern.
- Maintain an accurate record of completed work and unresolved failures. Do not stop with a partial status report when authorized implementation can continue.
