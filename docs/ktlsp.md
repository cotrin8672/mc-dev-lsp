# Rust integration with ktlsp

The `codex/ktlsp-rust` branch starts a Rust implementation alongside the existing
JDT LS extension. The original checkout stays on `main`; its uncommitted work is
not copied into this branch. This is an initial Mixin slice, not feature parity
with `mcdev-core`.

## Architecture and distribution

`mcdev-ktlsp` exports `McdevMixinProvider` and builds an LSP executable with that
provider registered through ktlsp's public `Backend::with_analysis_providers`.
The provider consumes ktlsp's normalized Java/Kotlin annotations, type identities,
and JVM member facts. It contributes member requirements and source sites to the
shared semantic engine. The host owns ordinary completion, diagnostics,
navigation, references, rename, project indexing, and invalidation.

The current host composes Rust crates at build time. A distributed
`mcdev-ktlsp` executable includes the host and provider; an end user can run a
prebuilt executable without compiling it. There is currently no protocol for
connecting a separate extension executable to a stock ktlsp process, and no
dynamic library loader. Running two LSP servers does not share their semantic
indexes. The provider remains a library so its registration can change when the
host gains another loading mechanism.

The Cargo manifests and lockfile pin ktlsp and its patched dependencies to a
specific Git revision. No local ktlsp checkout or machine-specific dependency path
is needed to build. Do not replace the revision with a moving branch while the
extension API is evolving.

The initial pin is `646d8277e22e53de1a9842c2a74b700f88dc28fb`, the checkpoint
recommended by the host maintainer for this integration.

## Initial scope

- Recognize the actual fully qualified Mixin and Shadow annotation types through
  the host resolver, including Java and Kotlin source.
- Resolve one target expressed as a class literal, a target string, or a
  single-element annotation array.
- Bind declared shadow methods and fields by exact host-provided JVM signature
  and static/instance dispatch.
- Preserve method prefixes and Kotlin identifier quoting when editing names.
- Preserve unknown results when the host cannot prove a type or generated member;
  unsupported cases must not bind to a guessed member.

The annotation contracts follow upstream Mixin 0.8.7's
[`Mixin`](https://github.com/SpongePowered/Mixin/blob/0.8.7/src/main/java/org/spongepowered/asm/mixin/Mixin.java)
and
[`Shadow`](https://github.com/SpongePowered/Mixin/blob/0.8.7/src/main/java/org/spongepowered/asm/mixin/Shadow.java)
declarations. In particular, `value` holds class literals, `targets` holds fully
qualified names, and prefix stripping applies to methods.

Shadow aliases are deferred. Kotlin accessor use-site annotations and JVM shapes
the host cannot normalize remain incomplete. A field whose name starts with its
shadow prefix is invalid under Mixin's contract and must not produce a binding.

MixinExtras, Lapis-generated members, injector selectors, `@At`, accessors,
invokers, overwrite validation, mappings/remapping, Access Widener, and Access
Transformer are not ported in this slice. Multiple Mixin targets and inherited
target members require further work. Resolving a string target for member
analysis does not add class navigation or rename within that string.

The current public API resolves the authored owner's name in the source context
and verifies the result against the exact host declaration identity. An
ambiguous or unsupported owner remains incomplete. A direct declaration-to-type
API was requested from the ktlsp maintainer to improve this coverage. No package
configuration or independent source parser is used.

The pinned host has a closed diagnostic-code enum. Provider diagnostics therefore
use namespaced message prefixes such as `[mcdev.mixin.incomplete]` with no custom
LSP diagnostic code. Structured extension codes were requested from the host.

## Build and Neovim setup

Install a current stable Rust toolchain. From the repository root:

```powershell
rtk cargo build --release --locked -p mcdev-ktlsp
```

The executable is `target/release/mcdev-ktlsp` (`.exe` on Windows). It speaks
standard LSP over stdio. Logs go to stderr. Configure it as the Java/Kotlin server
in your editor. For Neovim 0.11 or later:

```lua
vim.lsp.config("mcdev_ktlsp", {
  cmd = { "/absolute/path/to/mcdev-ktlsp" },
  filetypes = { "java", "kotlin" },
  root_markers = { "settings.gradle.kts", "settings.gradle", "pom.xml", ".git" },
})
vim.lsp.enable("mcdev_ktlsp")
```

Use this executable for those buffers in place of JDT LS or stock ktlsp. The
existing `mcdev-nvim` completion sources and `mcdev.*` execute commands belong to
the JDT LS adapter; the Rust server uses the editor's ordinary LSP integration.

The registered provider ID is `mcdev.mixin`. ktlsp's `analysisProviders` setting
can disable or re-enable it through initialization options or
`workspace/didChangeConfiguration`:

```json
{
  "analysisProviders": {
    "mcdev.mixin": { "enabled": false, "configuration": {} }
  }
}
```

## Verification record

The Rust suite exercises the provider through ktlsp's real workspace API. The
editor smoke uses a disposable project and real Neovim LSP requests. Reduced
annotation declarations in these tests validate analysis behavior; they do not
prove Mixin transformation, a Minecraft launch, production remapping, or compiler
compatibility with every supported mod loader.

The editor smoke also supplies a minimal Java SDK source archive through the
host's `KTLSP_JDK_SRC` override. It verifies primitive-signature LSP behavior,
not a full SDK or compiler. It waits for the host's project/library indexing
notifications before sending each semantic request once. Fixtures live outside
the Git checkout because the host respects Git ignore rules; their original
sources and actual server exit status are retained in `editor-proof.json`.

Verification requires PowerShell 7 and Neovim 0.11 or later:

```powershell
rtk proxy pwsh -NoProfile -File scripts/run-rust-tests-compact.ps1
```

The runner retains complete logs under `build/rust-test-logs`, prints compact
summaries, and terminates its owned process tree on timeout. Rust verification
is separate from the existing Gradle and JDT LS checks.

| Acceptance case | Current evidence |
| --- | --- |
| Java/Kotlin exact shadow binding and nominal descriptors | Passed in the 13-test Workspace suite |
| Conservative handling of unsupported or ambiguous inputs | Passed in the Workspace suite |
| Unsaved edits and provider configuration invalidate contributions | Workspace edits and real-editor disable/re-enable checks passed |
| Standard LSP navigation and references in real Neovim | Passed: 49 checks across 20 RPCs, including the exact five-location reference set |
| Rename preserves prefixes and unrelated source | Passed by applying Java/Kotlin rename edits and inspecting source |
| Compact runner detects failures and timeouts | Passed: nonzero exit, retained streams, timeout, owned descendant termination |

Verified on Windows with the pinned host revision above. The final compact run
passed formatting, all Rust targets (13 tests), the executable build, and the
49-check editor smoke with clean server exit `0`. Complete logs and the editor
proof are under `build/rust-test-logs/20261023-212917410/`; disposable fixture
cleanup also passed. The editor was independently rerun under
`build/rust-test-logs/editor-lifecycle-20261023-212704742/`. The runner self-check
passed under `build/rust-test-logs/runner-check-20261023-212704924/`.

The separate Rust CI workflow runs the same checks on Ubuntu and Windows. That
hosted CI run, the original Gradle/JDT LS suite, and a release-profile build were
not run as part of this initial Rust slice.

After this slice, the next steps are direct declaration identity support in the
host, broader Mixin target/member coverage, and migration of injector semantics.
