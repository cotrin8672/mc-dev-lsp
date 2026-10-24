# Stock-host opaque-configuration regression

`stock_f595_array_configuration.json` is the actual initialize/evaluate input
captured from stock ktlsp `f5953350928eb8af1ab34bb39db23ba7370d14af` with the
reduced Neovim Java/Kotlin fixture. The owner, UTF-8 ranges, source and opaque
handles are preserved. It contains `configuration: []`, which Neovim sends for
an empty Lua table. No filesystem lookup is made from the recorded owner path.

The old immutable MC executable rejected this valid opaque JSON setting with
`-32602: invalid evaluation`. Replaying the exact recorded stdin into that
executable produced byte-identical stdout and empty stderr. The host then closed
the transport and reported `extension.ipc.exited`/`Stale`; those events were a
secondary symptom, not a child crash. The semantic-v1 contract and stock host
pass configuration as opaque JSON. The current MC policy does not interpret it.

The production fix removes only the object-kind restriction in
`wire::Evaluation::validate`. Initialization, framing, source ranges/spelling,
opaque-handle ownership/type checks, cancellation and generation guards remain.
The process regressions prove the real `[]` input starts the expected scoped fact
queries and that object, array, null, boolean, number and string settings all
preserve normal evaluation. A deliberately unavailable fact reply remains
Unknown and never becomes a successful target binding.

`stdio_record_proxy.rs` is a test-only byte-for-byte stdin/stdout passthrough with
separate child stderr recording. The MC executable has no recording path, test
marker, timing barrier, filesystem write or extra LSP connection.
`freeze_consumer.py` checks all source/Cargo hashes before and after building and
copies the binary into an immutable proof directory. `interop_receipt.json`
records binary hashes, source receipt and full log locations.

The final real proof used that immutable MC executable and the unchanged stock
host. It passed 112 checks and 29 single-attempt RPCs, including the original 20,
in 25.7 seconds under a 120-second deadline, with clean shutdown. It verified:

* Exact Java/Kotlin goto and references; disable/re-enable requires a newly
  accepted instance, matching source hashes and distinct evaluation tokens.
* Five real rename edits applied to the target and projected declaration/call
  sites, preserving the other overload, Java prefix and Kotlin backticks.
* An unsaved Java signature change to `(J)J`, fresh same-instance/hash acceptance,
  exact long-overload goto/references and withdrawal from the int family.
* Java `(J)J` and Kotlin `(I)I` completion projections: exact descriptor detail,
  label/filter/sort/insert/kind/format, editable suffix range and text, followed
  by real edit application and whole-buffer comparison. Kotlin surface labels
  intentionally retain backticks. Copies are applied so recorded RPC responses
  remain unmodified.

The filesystem observer has its own focused test for provider/source filtering,
partial records, old/fresh instances, mixed-instance sets, duplicate/replayed
evaluation tokens, required-instance scope and event freshness. All 165 native
companion tests passed, including the existing 163. The editor input scripts and
both executable hashes remained unchanged during the final run.

This is the reduced semantic-v1 source Shadow/method slice. It does not establish
full Minecraft behavior, all JDT E2E cases, or command/resource/classpath-capability
reachability. Host failure-cause observability is a separate host-side follow-up.
