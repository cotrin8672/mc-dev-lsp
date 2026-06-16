# Architecture

## Architectural Summary

The system has three layers:

```text
Neovim UI layer
  Lua plugin, completion source, commands, request wrapper

JDT LS integration layer
  OSGi bundle, JDT model, classpath, AST, binding, LSP conversion

Core semantic layer
  Kotlin domain logic for descriptors, mappings, bytecode, Mixin, MixinExtras, AW, AT
```

The most important rule is that the core semantic layer must be editor-independent.

## Runtime Flow

Typical completion request:

```text
Neovim buffer
  -> mcdev.nvim source
  -> workspace/executeCommand or supported JDT LS extension command
  -> mcdev-jdtls-extension command handler
  -> JDT project/context lookup
  -> ProjectContext
  -> mcdev-core semantic service
  -> domain completion items
  -> LSP/Neovim completion response
  -> Blink/cmp UI
```

Typical diagnostic flow:

```text
JDT LS document/project event
  -> mcdev-jdtls-extension context builder
  -> ProjectContext
  -> mcdev-core diagnostic services
  -> diagnostic conversion
  -> publish diagnostics
```

Typical code action flow:

```text
Neovim code action request or explicit mcdev command
  -> extension command/code action handler
  -> context and domain diagnostic lookup
  -> mcdev-core suggests domain fix
  -> extension converts fix into WorkspaceEdit
  -> Neovim applies edit through LSP
```

## Kotlin/JVM Baseline

The project targets:

```text
Java runtime: 21
Kotlin JVM target: 21
```

Gradle Kotlin DSL should configure:

```kotlin
kotlin {
    jvmToolchain(21)
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
        freeCompilerArgs.add("-Xjsr305=strict")
    }
}
```

Do not introduce Kotlin reflection unless there is a documented need.

Avoid coroutines by default. Prefer:

- Eclipse Jobs
- `ExecutorService`
- bounded caches
- synchronous core services

Reason:

- JDT/Eclipse APIs are largely synchronous
- OSGi classloader behavior is easier to debug with fewer runtime dependencies
- completion latency matters more than async abstraction style

## OSGi Bundle Model

`mcdev-jdtls-extension` must be a real OSGi bundle, not a generic fat jar.

The bundle must define:

```text
Bundle-SymbolicName: io.github.mcdev.jdtls
Bundle-Name: Minecraft Development for JDT LS
Bundle-Version: x.y.z
Bundle-Activator: io.github.mcdev.jdtls.McdevPlugin
```

Dependencies should be handled deliberately:

- embed Kotlin stdlib
- do not use Kotlin reflect
- avoid kotlinx.coroutines
- embed or privately package `mcdev-core`
- avoid leaking internal packages into the OSGi environment
- avoid accidentally importing incompatible ASM packages from the host

Preferred packaging direction:

```text
Bundle-ClassPath: ., lib/kotlin-stdlib.jar, lib/mcdev-core.jar, lib/asm.jar, ...
Private-Package: io.github.mcdev.*
Import-Package: org.eclipse.*, org.eclipse.jdt.*, org.eclipse.lsp4j.*, *
```

ASM is a special risk. If JDT LS or Eclipse already provides ASM, version conflicts are possible. The safest default is to keep the ASM used by `mcdev-core` private to the extension bundle unless a deliberate compatibility decision is made.

## Core Domain Packages

Recommended package structure:

```text
io.github.mcdev.core
├─ descriptor
├─ mapping
├─ bytecode
├─ mixin
├─ mixinextras
├─ aw
├─ at
├─ completion
├─ diagnostics
└─ codeaction
```

### descriptor

Responsibilities:

- parse JVM field descriptors
- parse JVM method descriptors
- parse owner/name/descriptor target strings
- render readable signatures
- render exact insertion text
- validate descriptors
- expose structured parse errors

Important files:

```text
JvmDescriptor.kt
DescriptorParser.kt
DescriptorRenderer.kt
MemberTargetParser.kt
```

### mapping

Responsibilities:

- represent namespaces
- parse Tiny v2
- parse SRG/TSRG as needed
- remap classes
- remap fields
- remap methods
- remap descriptors
- report missing mapping entries

Important files:

```text
MappingNamespace.kt
MappingResolver.kt
TinyV2Parser.kt
SrgParser.kt
DescriptorRemapper.kt
ProjectMappingContext.kt
```

### bytecode

Responsibilities:

- provide class bytes
- read classes with ASM
- index class members
- extract instruction candidates for injection points
- calculate ordinals per target type
- classify constants, field accesses, invokes, constructors, loads/stores, jumps, returns

Important files:

```text
BytecodeIndex.kt
ClasspathClassProvider.kt
InjectionCandidateExtractor.kt
OrdinalCalculator.kt
InstructionDisplayRenderer.kt
```

### mixin

Responsibilities:

- model Mixin annotations
- resolve Mixin target classes
- resolve injector method attributes
- resolve `@At` context
- validate shadows/accessors/invokers
- produce diagnostics and code actions

Important files:

```text
MixinAnnotationModel.kt
MixinTargetResolver.kt
InjectorModel.kt
AtTargetResolver.kt
MixinDiagnostics.kt
MixinCodeActions.kt
```

### mixinextras

Responsibilities:

- model MixinExtras annotations
- support `MIXINEXTRAS:EXPRESSION`
- generate/fix handler signatures
- validate `Operation<T>` parameters
- validate expression-specific constraints

Important files:

```text
MixinExtrasAnnotationModel.kt
HandlerSignatureService.kt
ExpressionSupport.kt
OperationSignatureRenderer.kt
```

### aw

Responsibilities:

- parse Access Widener files
- identify completion slots
- complete directives
- complete classes
- complete members
- validate descriptors
- detect duplicates
- remap namespace where supported

Important files:

```text
AccessWidenerParser.kt
AccessWidenerModel.kt
AccessWidenerCompletion.kt
AccessWidenerDiagnostics.kt
AccessWidenerCodeActions.kt
```

### at

Responsibilities:

- parse Access Transformer files
- complete modifiers
- complete classes
- complete members
- insert SRG or project-required namespace
- validate method descriptors
- detect duplicates
- report missing SRG mappings

Important files:

```text
AccessTransformerParser.kt
AccessTransformerModel.kt
AccessTransformerCompletion.kt
AccessTransformerDiagnostics.kt
AccessTransformerCodeActions.kt
```

## JDT LS Extension Packages

Recommended package structure:

```text
io.github.mcdev.jdtls
├─ McdevPlugin.kt
├─ command
├─ protocol
├─ project
├─ java
├─ completion
├─ diagnostics
├─ definition
├─ references
├─ codeaction
├─ index
└─ logging
```

### project

Owns conversion from Eclipse/JDT world to `ProjectContext`.

Important files:

```text
JdtProjectModelService.kt
JdtClasspathService.kt
PlatformDetector.kt
ProjectContextService.kt
MappingDiscoveryService.kt
MixinConfigDiscoveryService.kt
AwAtDiscoveryService.kt
```

### java

Owns Java AST and binding operations.

Important files:

```text
JdtAstService.kt
JdtBindingService.kt
JdtDescriptorService.kt
JdtSearchService.kt
AnnotationContextExtractor.kt
```

### command

Owns Neovim-facing commands.

Important command names:

```text
mcdev.completion
mcdev.definition
mcdev.references
mcdev.codeAction
mcdev.diagnostics
mcdev.reindex
mcdev.context (compatibility alias for diagnostics)
mcdev.info
```

Do not assume arbitrary custom LSP methods are supported. The default design should work through command execution unless a stable JDT LS extension point is confirmed and documented.

## Core Data Model

Use explicit namespace-carrying references.

```kotlin
enum class MappingNamespace {
    NAMED,
    INTERMEDIARY,
    OFFICIAL,
    SRG,
    MCP,
}

enum class MemberKind {
    CLASS,
    METHOD,
    FIELD,
    CONSTRUCTOR,
}

data class ClassRef(
    val internalName: String,
    val namespace: MappingNamespace,
)

data class MethodRef(
    val owner: ClassRef,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)

data class FieldRef(
    val owner: ClassRef,
    val name: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)
```

Never pass a raw string when the string's namespace matters.

## Completion Model

Core completion model:

```kotlin
data class McCompletionItem(
    val label: String,
    val detail: String?,
    val documentation: String?,
    val filterText: String,
    val insertText: String,
    val kind: McCompletionKind,
    val sortKey: String,
    val metadata: McCompletionMetadata,
)
```

The LSP conversion belongs in `mcdev-jdtls-extension`, not in `mcdev-core`.

## Diagnostic Model

Diagnostics should be structured before they become LSP diagnostics.

```kotlin
data class McDiagnostic(
    val code: McDiagnosticCode,
    val severity: McSeverity,
    val message: String,
    val range: McTextRange,
    val metadata: McDiagnosticMetadata,
)
```

Structured metadata matters because code actions need to know what failed without reparsing the user-facing message.

## Code Action Model

Core should produce domain fixes, not LSP workspace edits.

```kotlin
sealed interface McFix {
    val title: String
}

data class ReplaceRangeFix(...)
data class AddMixinConfigEntryFix(...)
data class AddAccessWidenerEntryFix(...)
data class AddAccessTransformerEntryFix(...)
data class GenerateHandlerMethodFix(...)
```

`mcdev-jdtls-extension` converts `McFix` to LSP `WorkspaceEdit`.

## Error Handling

The extension must distinguish:

- no applicable context
- incomplete project context
- missing mapping data
- missing classpath entry
- bytecode unavailable
- parse error
- internal exception

Completion may return an empty list for no applicable context.

Completion should return structured warning metadata for incomplete project context when that explains missing items.

Diagnostics should report project-level problems when they affect semantic quality:

```text
mapping not loaded
classpath incomplete
target class not compiled
descriptor parse failed
bytecode missing
```

Do not hide these silently.
