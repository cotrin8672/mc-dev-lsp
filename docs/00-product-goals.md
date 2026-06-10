# Product Goals

## Product Definition

`mcdev-kotlin` is a Minecraft modding semantic extension for JDT LS with a Neovim frontend.

The intended user experience is close to IntelliJ IDEA with MinecraftDev for the selected surfaces:

- intelligent class/member/method target completion
- mapping-aware insertion
- JVM descriptor-aware insertion
- bytecode-aware `@At(target = "")` completion
- Mixin and MixinExtras handler signature assistance
- diagnostics for unresolved or ambiguous modding targets
- definition and reference navigation across Java, Mixin annotations, Access Widener, Access Transformer, and Mixin config JSON
- quick-fix style workspace edits

The product should feel like Minecraft-specific Java semantics were added to JDT LS, not like a text snippet plugin.

## Supported Surfaces

The supported semantic surfaces are:

- Mixin
- MixinExtras
- Access Widener
- Access Transformer
- Mixin config JSON when required by Mixin diagnostics and code actions

The following are intentionally out of scope:

- NBT editor
- project creator
- GUI wizard
- debugger position manager
- run configuration generation
- asset browser
- datagen runner
- Minecraft version installer

## Target Experience

The user should be able to work in Neovim and get the following behavior.

### Mixin Target Completion

When editing:

```java
@Mixin()
class MinecraftClientMixin {
}
```

completion should show readable classes:

```text
MinecraftClient       net.minecraft.client
GameRenderer          net.minecraft.client.render
DrawContext           net.minecraft.client.gui
```

and insert one of:

```java
MinecraftClient.class
```

or, depending on configuration:

```java
net.minecraft.client.MinecraftClient.class
```

If imports are added, they must be added through deterministic additional text edits.

### String Target Completion

When editing:

```java
@Mixin(targets = "")
```

completion should display readable class names but insert binary or fully qualified target strings expected by Mixin:

```text
net.minecraft.client.MinecraftClient
```

### Injector Method Completion

When editing:

```java
@Inject(method = "")
```

completion should show methods from the resolved Mixin target:

```text
tick(): void
render(float, long, boolean): void
setScreen(Screen): void
```

Insertion rules:

- if no overload exists, insert method name
- if overload exists, insert descriptor-qualified target
- if configured as `always`, always insert descriptor-qualified target
- if configured as `never`, insert method name only but report ambiguity when needed

Examples:

```java
@Inject(method = "tick")
@Inject(method = "render(FJZ)V")
```

### At Value Completion

When editing:

```java
@At(value = "")
```

completion should include:

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

MixinExtras values are first-class supported values, not plugin-specific afterthoughts.

### At Target Completion

When editing:

```java
@At(value = "INVOKE", target = "")
```

the extension must inspect the target method bytecode and list invocation candidates.

Display:

```text
draw(String, float, float, int): int          TextRenderer
drawTexture(Identifier, int, int, int, int)   DrawContext
enableBlend(): void                           RenderSystem
```

Insertion:

```text
Lnet/minecraft/client/font/TextRenderer;draw(Ljava/lang/String;FFI)I
Lnet/minecraft/client/gui/DrawContext;drawTexture(Lnet/minecraft/util/Identifier;IIII)V
Lcom/mojang/blaze3d/systems/RenderSystem;enableBlend()V
```

The visible text and inserted text must be separate fields in the completion model.

### Access Widener Completion

When editing:

```text
accessible method net/minecraft/client/MinecraftClient 
```

completion should show readable member information:

```text
tick(): void
setScreen(Screen): void
currentScreen: Screen
```

and insert:

```text
setScreen (Lnet/minecraft/client/gui/screen/Screen;)V
```

The final line should be:

```text
accessible method net/minecraft/client/MinecraftClient setScreen (Lnet/minecraft/client/gui/screen/Screen;)V
```

### Access Transformer Completion

When editing:

```text
public net.minecraft.client.Minecraft 
```

completion should show named or readable information:

```text
setScreen(Screen): void       named: setScreen
player: ClientPlayerEntity    named: player
```

and insert the namespace required by the project:

```text
m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
f_91074_
```

The final line can be:

```text
public net.minecraft.client.Minecraft m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V
```

## Goal-State Acceptance Conditions

The product is considered aligned with this design when all of these are true:

1. `@At(target = "")` can display readable bytecode candidates and insert exact JVM target descriptors.
2. `@Inject(method = "")` can complete target methods and insert descriptors when overloads require them.
3. `@Mixin`, `@Shadow`, `@Accessor`, and `@Invoker` can complete targets and navigate to definitions.
4. MixinExtras annotations are supported at the same quality level as core Mixin annotations for method targets, `@At`, and handler signatures.
5. Access Widener class/member/descriptor completion works with mapping-aware output.
6. Access Transformer completion displays readable names and inserts SRG or the required namespace.
7. Diagnostics are produced for unresolved targets, invalid descriptors, ambiguous targets, out-of-range ordinals, duplicate AW/AT entries, and handler signature mismatches.
8. Code actions can update Java, AW, AT, and Mixin config JSON files through minimal workspace edits.
9. Neovim contains no semantic logic beyond transport and UI adaptation.

## Product Non-Negotiables

- Completion must not scan jars synchronously.
- Neovim must not resolve mappings.
- Diagnostics must not silently disappear when the extension has partial context.
- Mapping namespace must be explicit in all target/member models.
- Every target insertion must be deterministic.
- Every workspace edit must be testable without launching Neovim.

