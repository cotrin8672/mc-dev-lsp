# Project Context And Indexing

## ProjectContext Is The Center

Most feature failures will come from weak project context, not weak completion UI.

The extension must build a single source of truth:

```kotlin
data class ProjectContext(
    val projectId: String,
    val root: Path,
    val platform: ModPlatform,
    val javaProject: JdtJavaProjectRef,
    val classpath: ClasspathSnapshot,
    val mappings: ProjectMappingContext,
    val mixinConfigs: List<MixinConfigRef>,
    val accessWideners: List<AccessWidenerRef>,
    val accessTransformers: List<AccessTransformerRef>,
    val minecraftJars: List<Path>,
    val sourceSets: List<SourceSetContext>,
    val indexState: ProjectIndexState,
)
```

Everything should flow through this model:

- completion
- diagnostics
- definition
- references
- code actions
- info command
- indexing

## Platform Detection

Supported platforms:

```text
Fabric
Forge
NeoForge
Unknown
```

Detection sources:

- Gradle files
- project dependencies
- mod metadata files
- known plugin IDs
- classpath markers

Platform detection must be explicit but tolerant. A project can be partially configured and still benefit from completion where enough context exists.

## Mapping Context

Mapping context must describe:

```kotlin
data class ProjectMappingContext(
    val sourceNamespace: MappingNamespace,
    val runtimeNamespace: MappingNamespace,
    val awNamespace: MappingNamespace?,
    val atNamespace: MappingNamespace?,
    val availableNamespaces: Set<MappingNamespace>,
    val resolver: MappingResolver,
)
```

Do not infer namespace from raw string shape alone.

Examples:

- Fabric source may display named and output intermediary depending on context.
- Access Widener declares its namespace.
- Forge Access Transformer may need SRG for member insertion.
- Java source may use named symbols but target strings may need descriptor or obfuscated/SRG names.

## Classpath Snapshot

The classpath snapshot must include:

- project output directories
- dependency jars
- Minecraft jars
- generated sources/output if relevant
- timestamps or content hashes

The extension should never scan the full classpath during a completion request.

## ProjectIndex

Recommended shape:

```text
ProjectIndex
├─ class index
├─ method index
├─ field index
├─ mapping index
├─ mixin config index
├─ AW/AT index
└─ bytecode instruction index
```

### Class Index

Stores:

- internal name
- qualified name
- simple name
- namespace
- source location if available
- classpath entry
- superclass/interfaces where cheaply available

### Member Index

Stores:

- owner class
- member name
- descriptor
- namespace
- static flag
- visibility
- source location if available
- classpath location

### Mapping Index

Stores bidirectional lookup:

- class mapping
- field mapping
- method mapping
- descriptor remapping

Mappings must be immutable snapshots for safe concurrent reads.

### Mixin Config Index

Stores:

- config file path
- package
- mixin class entries
- client/server/common groups
- JSON ranges for edits if available

This enables:

- diagnostics for missing mixin config entries
- code action to add class to the correct config
- definition from config string to mixin class

### AW/AT Index

Stores:

- parsed entries
- ranges
- duplicates
- target refs
- namespace

This enables:

- diagnostics
- references
- duplicate detection
- safe edit generation

### Bytecode Instruction Index

Stores per method:

- invokes
- field accesses
- new instructions
- constants
- returns
- jumps
- loads/stores if needed
- ordinal groups

Cache key:

```kotlin
data class BytecodeCacheKey(
    val classpathEntry: Path,
    val lastModified: Long,
    val internalName: String,
)

data class MethodInstructionCacheKey(
    val classRef: ClassRef,
    val methodName: String,
    val descriptor: String,
    val namespace: MappingNamespace,
)
```

If the classpath entry timestamp changes, invalidate affected bytecode.

## Completion Performance Rules

Completion must:

- read indexes
- use current AST/buffer context
- do small filtering and rendering
- avoid full jar scanning
- avoid network
- avoid Gradle invocation
- avoid blocking long operations

Completion may:

- use cached bytecode instruction lists
- trigger background indexing when cache is missing
- return partial results with warnings

Completion must not:

- build the project
- run Gradle
- download dependencies
- parse all classpath jars on demand

## Invalidation

Invalidate project context when:

- classpath changes
- mapping files change
- Gradle project model changes
- mixin config files change
- AW/AT files change
- output class files change
- source file relevant to current target changes

Use coarse invalidation first. Optimize only after correctness is stable.

## Concurrency

Recommended model:

- immutable snapshots for indexes
- one active snapshot per project
- background rebuild into a new snapshot
- atomic swap after successful rebuild
- completion reads the latest complete snapshot

Do not mutate shared maps in place while completion reads them.

## Partial Context Behavior

When context is incomplete:

```text
mapping missing        -> class completion may work, namespace insertion may not
bytecode missing       -> method completion may work, @At target completion may not
mixin config missing   -> Mixin target completion works, config quickfix unavailable
AW namespace unknown   -> AW diagnostics should report namespace problem
AT SRG missing         -> AT should report SRG mapping not found
```

The system should degrade explicitly, not silently.

## McdevInfo Fields

`:McdevInfo` should be backed by `ProjectContext`.

Suggested output:

```text
Project: fabric
Root: /path/to/project
Platform: Fabric
Mappings: named <-> intermediary loaded
Source namespace: named
Runtime namespace: intermediary
Minecraft jar: found
Mixin config: 2 files
Access Widener: 1 file
Access Transformer: none
Classpath entries: 142
Class index: ready
Bytecode index: ready
Protocol: 1
Extension: 0.1.0
```

