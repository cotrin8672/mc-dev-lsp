# Slice 143 isolated companion adapter evidence

Status: proposed two-operation contract, compiled adapter; no live host capability
or production negotiation. Current host `f76a43fbeddf4fbdc69768fd0f7544e5a4a3610f`
does not implement projectContext/typeJvm. See `docs/mcdev-host-capabilities.md`
for the primary draft and its SHA256. The implemented host serializer/fixed commit
will be the final source for this experimental wire.

Production adapter files: `src/host_facts/{draft,bridge}.rs`. They expose only
optional-operation intersection, per-field project metadata, checked typeJvm,
raw ABI preservation and complete-header projection into existing core DTOs.
`Server` implements the borrowed FactReader through its existing request actor;
there is no second IO router, project scanner, class reader, generation authority,
read-set validator or full project/index provider.

All 13 adapter tests passed. The complete isolated companion check passed 180
tests (2 library, 135 command, 13 adapter, 10 lifecycle, 15 semantic, 5 framing):
exit 0, no timeout, 127.4 seconds against a 300-second deadline, full logs
`build/rust-test-logs/companion-isolated-20261024-171426037/companion.*.log`.
This fresh build uses task-owned `build/host-facts-target` and sequential test
functions. HTTP tests retain their original internal two-client parallelism,
120 ms callback, 1-second deadline and all original assertions.

An earlier shared-target check failed and remains recorded at
`build/rust-test-logs/companion-20261024-170848605/`: one HTTP request returned 504
instead of 200; rustdoc could not find its dependency rlib paths. Their causes
were not established. Neither failure reproduced in the isolated complete run;
no shared target was cleaned/deleted, timeout/input changed or assertion waived.

## Executable checks

| Rust test | Evidence |
|---|---|
| optional_operations_are_a_per_instance_intersection_with_legacy_fallback | Missing optional capability/map produces no operations; unsupported names remain unsupported; duplicate names are a set; malformed schema rejected. |
| project_metadata_keeps_independent_knowledge_and_classpath_order | Scope/classpath coverage Unknown preserves positive workspace and ordered observed entries; query has no project/path parameter. |
| sdk_full_release_and_unknown_are_distinct | Full API, release API and Unknown SDK remain separate states. |
| scoped_type_requires_a_host_issued_current_type_handle_before_io | Unissued type is rejected before IO; current type token is passed unchanged. |
| complete_editing_coverage_and_known_instance_do_not_establish_jvm_abi | Source/editing coverage and known instance dispatch cannot create a complete raw ABI. Editing handles stay in the current registry. |
| complete_raw_abi_retains_generated_overloads_initializers_and_exact_flags | Exact full-object round trip; six raw methods, two fields, constructor/clinit, same-name overloads, bridge 0x1041 versus 0x0001, and Synthetic attribute without a synthetic raw flag. |
| raw_header_coverage_needs_matching_binary_owner_and_exact_digest | Wrong owner, bad binary descriptor, uppercase digest, incomplete ABI coverage and malformed method descriptor rejected by the appropriate boundary. |
| raw_members_cannot_receive_fabricated_editing_ids | Raw method objects cannot carry invented editing IDs. |
| full_metadata_and_abi_still_do_not_supply_project_sessions_or_executable_code | No full HostSessionIdentity or Code/LVT provider; existing dispatcher/delegate preserve INCOMPLETE_PROJECT_CONTEXT. |
| host_cancelled_or_stale_reads_never_return_partial_facts | Host stale/cancel signals propagate without returning facts. This tests propagation, not a child-side invalidation algorithm. |
| malformed_cardinality_and_unknown_reasons_are_rejected | Exact fact count and nonempty structured Unknown reasons required. |
| two_ops_process_worker | Native worker entry point plus default no-operations guard. |
| native_two_op_process_projection_reaches_core_and_dto_without_a_fake_session | Three real Rust child-process scenarios: complete header → existing descriptor/core DTOs → protocol converter; source-only → incomplete context; stale → cancellation. UTF-8 byte edits become UTF-16 2..5. |

The additional library test
`server::tests::optional_scoped_reader_uses_existing_cancel_and_late_reply_router`
uses the real existing actor: cancel e1, ignore its retired late RPC reply, then
read e2 facts. Existing V1 tests retain global framing/batch/deadline/lifecycle
proof. The command HTTP regression
`unavailable_facts_are_http_200_business_errors_and_authentication_stays_400`
keeps unavailable business envelopes separate from authentication rejection.

## Fixture limits and pending acceptance

`tests/two_ops_support` is explicitly synthetic wire data, not a captured host
ClassFileFacts projection, compiled Person/Model fixture, or source-callability
oracle. It has an authored value method plus raw generated-style getter,
initializer and overload entries, with no fabricated raw MemberIds. Tests retain
all metadata rather than filtering generated members.

Native process frames, results and worker stdout/stderr are saved under
`build/two-ops-process-logs/`. The fixture protocol exists only inside the Rust
test executable; it adds no mode/method to the production companion. The DTO
projection check is not a new MC completion feature or connected command wire.

Still pending: implemented/published two-op host contract; exact real producer
projection fixtures; negotiation and policy attachment to that fixed host;
stock-host stale/model/namespace/compiled/SDK/classpath/provider lifecycle checks;
real host/editor class-fact acceptance. The host's existing pending ReadSet and
publication path must prove those invalidations. Full sessions, catalogs, bytes,
Code/LVT/annotation payloads, assignability, resources and commands remain later
capabilities. The rejected broader eight-counter prototype is archived only in
`build/host-facts143-superseded` and is not counted here.
