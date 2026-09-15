# Domain Features

## Mixin

### Member Parser Status

`@Shadow`, `@Accessor`, `@Invoker`, and `@Overwrite` member declarations are currently parsed by a strengthened
JDT AST/binding-backed semantic model in JDT LS runtime, with a hand-written fallback parser for non-JDT paths. The fallback handles common multiline declarations, parameter annotations, generic erasure, arrays,
varargs, explicit imports, `java.lang` types, selected well-known Mixin/Minecraft types, and class-index-confirmed
same-package or wildcard imports.

When JDT AST APIs are unavailable, unresolved or ambiguous Java types are reported as diagnostics instead of
being converted to guessed JVM descriptors, and definition/code-action paths are expected to stop when the handler
descriptor cannot be resolved safely.

### Supported Annotations

Core Mixin annotations:

```text
@Mixin
@Shadow
@Overwrite
@Accessor
@Invoker
@Inject
@Redirect
@ModifyArg
@ModifyArgs
@ModifyVariable
@ModifyConstant
```

### @Mixin

Supported forms:

```java
@Mixin(MinecraftClient.class)
@Mixin(value = MinecraftClient.class)
@Mixin({ MinecraftClient.class, GameRenderer.class })
@Mixin(targets = "net.minecraft.client.MinecraftClient")
@Mixin(targets = { "a.b.C", "x.y.Z" })
```

Features:

- class completion
- import insertion or FQN insertion
- unresolved target diagnostics
- definition navigation to target class
- references from target class to mixins
- code action to add mixin class to config JSON

Diagnostics:

```text
unresolved @Mixin target
mixin class not listed in config
duplicate mixin target entry
invalid target string
```

### @Shadow

Supported forms:

```java
@Shadow private int field;
@Shadow public abstract void method();
@Shadow(remap = false) private int externalField;
@Shadow(prefix = "shadow$") public abstract void shadow$method();
```

Features:

- field/method target completion
- descriptor-aware validation
- prefix-aware resolution
- definition to target member
- references from target member to shadow declarations

Diagnostics:

```text
@Shadow target not found
@Shadow descriptor mismatch
@Shadow static mismatch
@Shadow field/method kind mismatch
```

### @Accessor

Supported forms:

```java
@Accessor("currentScreen")
Screen getCurrentScreen();

@Accessor
Screen getCurrentScreen();

@Accessor("currentScreen")
void setCurrentScreen(Screen screen);
```

Features:

- field completion
- getter/setter name inference
- generated method code action
- definition to field
- diagnostic for missing field
- diagnostic for wrong return/parameter shape

### @Invoker

Supported forms:

```java
@Invoker("setScreen")
void invokeSetScreen(Screen screen);

@Invoker
void invokeSetScreen(Screen screen);
```

Features:

- method completion
- name inference from invoker method name
- descriptor validation
- generated method code action
- definition to target method

### Injector method = ""

Supported annotations:

```text
@Inject
@Redirect
@ModifyArg
@ModifyVariable
@ModifyConstant
@ModifyExpressionValue
@ModifyReturnValue
@WrapOperation
@WrapWithCondition
@WrapMethod
```

Features:

- target method completion from resolved Mixin class
- overload detection
- descriptor insertion based on configuration
- unresolved method diagnostics
- ambiguous method diagnostics
- code action to add descriptor

### @At(value = "")

Supported values:

```text
HEAD
RETURN
TAIL
INVOKE
INVOKE_ASSIGN
FIELD
NEW
CONSTANT
JUMP
LOAD
STORE
MIXINEXTRAS:EXPRESSION
```

### @At(target = "")

Candidate extraction depends on `@At(value)`.

#### INVOKE

Read target method bytecode and list invocation instructions.

Insertion:

```text
Lowner/InternalName;methodName(methodDescriptor)returnDescriptor
```

Diagnostics:

```text
unresolved @At target
invalid method target descriptor
target instruction not present
ordinal out of range
```

#### FIELD

Read field access instructions.

Insertion:

```text
Lowner/InternalName;fieldName:fieldDescriptor
```

Diagnostics:

```text
invalid field target descriptor
invalid opcode for @At(value = "FIELD")
target field access not present
ordinal out of range
```

#### NEW

Read type creation instructions.

Insertion options:

```text
Lowner/InternalName;
Lowner/InternalName;<init>(...)V
```

Default should be class target unless configuration requests constructor descriptors.

#### RETURN

List return ordinals.

Completion may insert `RETURN` and provide an additional edit for `ordinal = n` when appropriate.

#### CONSTANT

List constants from bytecode:

```text
string
int
long
float
double
class literal
null
```

Code action/completion should help generate:

```java
@Constant(stringValue = "...")
@Constant(intValue = 0)
```

## MixinExtras

### Supported Annotations

```text
@ModifyExpressionValue
@ModifyReturnValue
@ModifyReceiver
@WrapOperation
@WrapWithCondition
@WrapMethod
@Local
@Share
@Definition
@Expression
```

### Handler Signature Service

`mcdev-core.mixinextras.HandlerSignatureService` owns all MixinExtras signature rules.

It must support at least:

- `@WrapOperation`
- `@ModifyExpressionValue`
- `@ModifyReturnValue`
- `@WrapWithCondition`
- `@WrapMethod`
- local capture parameters where applicable

Example `@WrapOperation`:

```java
private int wrapDraw(
    TextRenderer instance,
    String text,
    float x,
    float y,
    int color,
    Operation<Integer> original
) {
    return original.call(instance, text, x, y, color);
}
```

Required code actions:

```text
Generate WrapOperation handler
Fix WrapOperation handler signature
Generate ModifyExpressionValue handler
Fix ModifyExpressionValue handler signature
Generate ModifyReturnValue handler
Fix ModifyReturnValue handler signature
```

Diagnostics:

```text
MixinExtras handler signature mismatch
missing Operation parameter
wrong Operation generic type
wrong original value type
wrong return type
unsupported expression context
```

### Handler declaration completion

After a supported annotation is complete, invoke the `mcdev` completion source
and choose the appropriate generated handler or declaration candidate. Injector
items are labeled `Generate ... handler`; `@Accessor`, `@Invoker`, and
`@Overwrite` use declaration-specific labels. A bare `@Overwrite` has no
closing `)`, so it is offered after the annotation itself is complete. The item
inserts a native snippet: the entire method name is an empty first tab stop
(`${1}`), and the body is the final tab stop (`$0`). This lets the method name
be entered without retaining a generated prefix such as `cem$`. The completion
adds the member newline when the cursor is still after the annotation, and
reuses existing indentation when the cursor is already on the next blank member
line.

The declaration generator covers these injector contracts:

```text
Mixin:      @Inject, @Redirect, @ModifyArg, @ModifyArgs, @ModifyVariable, @ModifyConstant
MixinExtras: @ModifyExpressionValue, @ModifyReturnValue, @ModifyReceiver,
             @WrapOperation, @WrapWithCondition, @WrapMethod
Declarations: @Accessor, @Invoker, @Overwrite
```

`@Local`, `@Share`, and `@Cancellable` are handler parameter helpers; they do
not create a second method declaration. Accessor and Invoker instance forms end
in an abstract declaration, while static forms and Overwrite use a body
snippet. For Overwrite, the empty method-name tab stop must be changed to the
selected target method name; it cannot be an arbitrary rename. The generator
offers a signature only when the target method is uniquely resolved. Redirect
targets also need a resolved invocation, field operation, or constructor.
`@ModifyVariable` needs bytecode and a unique `LOAD`/`STORE` local after its
`index`, `ordinal`, `name`, or `argsOnly` constraints are applied. In ambiguous
or incomplete project contexts the completion item is omitted so it cannot
insert a guessed JVM signature. Minimal `@ModifyArg` and `@ModifyVariable`
stubs return the original value, while non-void `@Redirect` stubs throw
`UnsupportedOperationException` until their body is implemented. Constructor
`@Inject` generation is limited to `RETURN`. Constructor `@WrapOperation`
`NEW` generation is offered only when bytecode proves the allocation occurs
after mandatory `this()` or `super()` initialization; ambiguous or pre-
initialization allocations are omitted. Injectors requesting captured locals and Redirect array-length or
array-element points remain omitted until the bytecode model can describe those
contracts exactly.

The annotation-name snippets also include the no-argument Mixin markers
`@Final`, `@Mutable`, `@Pseudo`, `@SoftOverride`, and `@Surrogate`; they add the
correct official import and place the cursor after the annotation.

## Access Widener

### File Detection

Supported file patterns:

```text
*.accesswidener
*.aw
```

Project discovery should also inspect mod metadata where applicable.

### Supported Completion

Directive completion:

```text
accessible
extendable
mutable
natural
```

Kind completion:

```text
class
method
field
```

Class completion:

```text
MinecraftClient       net/minecraft/client/MinecraftClient
GameRenderer          net/minecraft/client/render/GameRenderer
```

Member completion:

```text
tick(): void
setScreen(Screen): void
currentScreen: Screen
```

### Diagnostics

```text
invalid directive
invalid kind
unresolved class
unresolved member
invalid descriptor
duplicate entry
mutable applied to non-field
extendable applied to invalid target
namespace mismatch
```

### Code Actions

```text
Generate Access Widener entry
Remap AW entry namespace
Add missing descriptor
Fix descriptor
Remove duplicate entry
```

## Access Transformer

### File Detection

Supported file patterns:

```text
*_at.cfg
accesstransformer.cfg
META-INF/accesstransformer.cfg
```

Project discovery should also inspect Forge/NeoForge build metadata where applicable.

### Modifier Completion

```text
public
protected
default
private
public-f
protected-f
private-f
public+f
protected+f
private+f
```

### Class And Member Completion

Class display:

```text
MinecraftClient       net.minecraft.client.MinecraftClient
GameRenderer          net.minecraft.client.render.GameRenderer
```

Member display:

```text
setScreen(Screen): void       named: setScreen
player: ClientPlayerEntity    named: player
```

Insertion depends on project mapping context:

```text
m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
f_91074_
```

### Diagnostics

```text
invalid modifier
unresolved class
unresolved member
missing descriptor for method
invalid descriptor
duplicate entry
wrong namespace
SRG mapping not found
```

### Code Actions

```text
Generate Access Transformer entry
Remap AT entry namespace
Add method descriptor
Fix descriptor
Remove duplicate entry
```

## Definition

Definition targets:

```text
@Mixin target              -> target class
@Inject method             -> target method
@At target                 -> target method/field/constructor
@Shadow                    -> target member
@Accessor / @Invoker       -> target member
AW class/member            -> Java class/member
AT class/member            -> Java class/member
Mixin config class string  -> mixin class
```

## References

References should include:

```text
target method -> related Mixin injectors
target field  -> @Shadow / @Accessor / AW / AT entries
class         -> mixin config / AW / AT entries
```

Reference search can be approximate initially only if diagnostics and completion remain exact. The target goal is exact project-local semantic references.
