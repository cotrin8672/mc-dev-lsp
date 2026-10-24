# semantic-v1 companion coverage

The source scenario set is `mcdev-ktlsp/tests/mixin.rs` (13 linked prototype tests).
Each corresponding function in `../semantic.rs` spawns the actual
`mcdev-companion` executable. `../support/semantic.rs` supplies explicit owned
normalized host facts and scoped opaque handles. It does not run a language
parser, JVM, project scanner or ktlsp/ktcore in the child. The mock host validates
batch/evaluation/declaration/signature/dispatch correspondence and decides the
binding; the companion cannot decide that a member resolved successfully.

| Linked test → identically named native process test | Executed assertions |
|---|---|
| `java_exact_methods_fields_dispatch_and_prefixes` | Five field/method/static/direct/default-prefix/custom-prefix bindings; exact names/name links; five sites; no incomplete/missing diagnostic. |
| `exact_signature_selects_the_correct_overload_location` | `(J)I` precedes `(I)I`; requirement retains `(I)I` and host returns the correct opaque target member. Actual goto location remains a host test. |
| `kotlin_nominal_types_arrays_and_backticks_use_shared_facts` | Four field/method bindings with nominal/array descriptors and `@field` use site; editable bytes select only `twice` inside backticks/prefix. |
| `single_class_and_fully_qualified_string_targets_bind` | All four Java and two Kotlin class/array/string target forms; one binding each. |
| `unsupported_or_multiple_targets_never_choose_a_first_target` | All five original unsupported/multiple forms; no requirement or binding; incomplete hint; no false missing-target diagnostic. |
| `exact_annotation_identity_is_required` | Fake Mixin and fake Shadow with matching qualified source spelling; no requirement/binding/missing-target diagnostic. Identity classification reuses `mcdev-core::mixin_source::MixinAnnotation`. |
| `descriptor_dispatch_unknown_and_aliases_do_not_guess` | All seven original return/static/unresolved-type/prefix/alias/generic negative forms plus Unknown target; Unknown never becomes missing. An additional DefinitelyAbsent target does produce missing. |
| `kotlin_accessors_and_unknown_abi_are_not_fields_or_plain_methods` | All six getter/setter/property/nullable-ABI forms remain incomplete and unbound without a false missing-target diagnostic. |
| `fields_do_not_strip_the_method_prefix` | Both original prefixed/empty-prefix field cases produce the field-prefix error and no requirement/binding. |
| `empty_effective_name_does_not_discard_other_contributions` | Invalid effective name emits a hint while the unrelated valid member still binds. |
| `ambiguous_owner_never_binds_to_another_declaration` | Host's authoritative declarationMember is Unknown; no name-based selection, requirement or missing-target diagnostic. |
| `unsaved_edits_and_provider_configuration_retract_and_rebind` | One persistent child evaluates original/changed-target/restored-target/changed-owner/restored-owner facts; a fresh child rebinds after closure. Real host disable/re-enable/retraction is parent-owned editor acceptance. |
| `rename_preserves_prefix_backticks_and_unrelated_source` | Java/Kotlin source with an unsaved Unicode comment and unrelated field; applying the returned editable-name projection preserves prefix/backticks/all unrelated bytes. Target-side rename orchestration remains host-owned. |

Additional native process regressions:

- 257 authored members cross both fact and requirement batch boundaries. The mock
  host enforces the published 256-item limit, and all 257 exact bindings survive.
- Cancellation while a requirement reply is outstanding discards both pending
  sites and already accumulated diagnostics; the old reply cannot contaminate a
  fresh evaluation in the same process.
- Eight lifecycle cases in `../lifecycle.rs` cover multiple evaluations,
  initialization/capability rejection, shutdown/exit/EOF, cancel/foreign cancel,
  late replies, Stale errors, foreign IDs, malformed Unknown facts and typed
  handle collisions. Five `../transport.rs` cases cover byte-exact UTF-8 framing,
  single-byte reads, consecutive frames, invalid/duplicate/oversized/truncated
  inputs and outgoing limits.
- A server unit test exercises expired evaluation and excess-batch rejection
  before any partial host request is written.

The production child advertises only `semantic-v1`. Passing this corpus does not
mean the entire frozen legacy mc-dev scope has an IPC implementation. The
declarationMember operation was verified from current host source; a published
revision containing it and the real stock-host/editor fixture remain separate
acceptance evidence, recorded by the parent task.

Final affected verification: `cargo test --offline --locked -p mcdev-companion
--no-fail-fast` through the compact runner, **29/29 passed**, exit 0, no timeout,
12.5 seconds under a 180-second deadline. Full command logs:
`build/rust-test-logs/companion-20261024-142937807/companion.{stdout,stderr}.log`.
Each child process's complete stderr is retained under
`build/companion-process-logs/`. Rustfmt and diff whitespace checks passed.

The tested executable was copied and its SHA-256 checked before and after the
copy: `build/companion-artifacts/semantic-v1-20261024-143050633/mcdev-companion.exe`,
SHA-256 `3e0fc4b9814ac0cf3699a5c3a62eddecb921f493ee32f7944307ed798e09f9c7`.
The adjacent `receipt.json` records source hashes and the successful validation;
`dependencies.stdout.log` records the normal dependency tree and confirms no
ktlsp or ktcore linkage. This copied binary is ready for parent-owned host/editor
validation, with a published checkpoint containing declarationMember still
required.
