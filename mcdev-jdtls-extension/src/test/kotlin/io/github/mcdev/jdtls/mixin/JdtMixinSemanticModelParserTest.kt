package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.MixinTargetRef
import io.github.mcdev.core.mixin.ParseSource
import io.github.mcdev.core.mixinextras.ExpressionContext
import io.github.mcdev.core.mixinextras.MixinExtrasDefinitionIndex
import io.github.mcdev.core.mixinextras.MixinExtrasExpression
import io.github.mcdev.core.mixinextras.MixinExtrasExpressionIndex
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import kotlin.test.assertTrue
import java.lang.reflect.InvocationTargetException
import java.util.concurrent.CancellationException

class JdtMixinSemanticModelParserTest {
    @Test
    fun reportsUnderlyingReflectionFailureInsteadOfInvocationWrapper() {
        val cause = IllegalStateException("AST create failed")

        assertEquals(
            "java.lang.IllegalStateException: AST create failed",
            JdtMixinSemanticModelParser.describeFailure(InvocationTargetException(cause)),
        )
    }

    @Test
    fun propagatesCancellationHiddenBehindReflectionWrapper() {
        val cancellation = CancellationException("cancelled")

        val thrown = assertFailsWith<CancellationException> {
            JdtMixinSemanticModelParser.throwIfJdtAbort(InvocationTargetException(cancellation))
        }

        assertSame(cancellation, thrown)
    }

    @Test
    fun fallsBackWithVisibleWarningWhenJdtAstIsUnavailableInUnitTests() {
        val source = """
            package com.example.mixin;
            @Mixin(SimpleTarget.class)
            class ExampleMixin {
                @Shadow private int counter;
            }
        """.trimIndent()

        val model = JdtMixinSemanticModelParser().parse(source, "file:///ExampleMixin.java")

        assertEquals("file:///ExampleMixin.java", model.sourceUri)
        assertEquals(ParseSource.HAND_WRITTEN_FALLBACK, model.parseSource)
        assertTrue(model.warnings.any { it.contains("JDT ASTParser is not available") })
    }

    @Test
    fun resolveJavaProjectReturnsNullWhenJdtWorkspaceIsUnavailable() {
        assertEquals(null, JdtMixinSemanticModelParser().resolveJavaProject("file:///ExampleMixin.java"))
    }

    @Test
    fun mixinTargetsUsesStringConstantInsteadOfStringExpressionBinding() {
        val extractorClass = JdtMixinSemanticModelParser::class.java.declaredClasses
            .single { it.simpleName == "AstModelExtractor" }
        val constructor = extractorClass.getDeclaredConstructor(String::class.java, String::class.java)
        constructor.trySetAccessible()
        val extractor = constructor.newInstance("", "file:///ExampleMixin.java")
        val mixinTargets = extractorClass.getDeclaredMethod("mixinTargets", Any::class.java)
        mixinTargets.trySetAccessible()

        @Suppress("UNCHECKED_CAST")
        val targets = mixinTargets.invoke(
            extractor,
            NormalAnnotation(
                fqn = "org.spongepowered.asm.mixin.Mixin",
                members = mapOf("targets" to StringLiteral("pkg.Target")),
            ),
        ) as List<MixinTargetRef>

        assertEquals(listOf("pkg/Target"), targets.map { it.internalName })
    }

    @Test
    fun recoveredTargetBindingUsesExplicitImportAndLaterUsesResolvedBinding() {
        val extractorClass = JdtMixinSemanticModelParser::class.java.declaredClasses
            .single { it.simpleName == "AstModelExtractor" }
        val constructor = extractorClass.getDeclaredConstructor(String::class.java, String::class.java)
        constructor.trySetAccessible()
        val mixinTargets = extractorClass.getDeclaredMethod("mixinTargets", Any::class.java)
        mixinTargets.trySetAccessible()
        for (binding in listOf(TypeBinding("Gui", recovered = true), TypeBinding("net.minecraft.client.gui.Gui"))) {
            val extractor = constructor.newInstance("import net.minecraft.client.gui.Gui;", "file:///GuiMixin.java")
            @Suppress("UNCHECKED_CAST")
            val targets = mixinTargets.invoke(extractor, SingleMemberAnnotation(
                "org.spongepowered.asm.mixin.Mixin", TypeLiteral(binding),
            )) as List<MixinTargetRef>
            assertEquals(listOf("net/minecraft/client/gui/Gui"), targets.map { it.internalName })
        }
    }

    @Test
    fun recoveredTargetBindingKeepsNestedSourceNameForSharedResolver() {
        val extractorClass = JdtMixinSemanticModelParser::class.java.declaredClasses
            .single { it.simpleName == "AstModelExtractor" }
        val constructor = extractorClass.getDeclaredConstructor(String::class.java, String::class.java)
        constructor.trySetAccessible()
        val mixinTargets = extractorClass.getDeclaredMethod("mixinTargets", Any::class.java)
        mixinTargets.trySetAccessible()
        val extractor = constructor.newInstance(
            "package example;\nimport sample.Outer;",
            "file:///NestedMixin.java",
        )

        @Suppress("UNCHECKED_CAST")
        val targets = mixinTargets.invoke(
            extractor,
            SingleMemberAnnotation(
                "org.spongepowered.asm.mixin.Mixin",
                TypeLiteral(TypeBinding("Outer.Inner", recovered = true), sourceName = "Outer.Inner"),
            ),
        ) as List<MixinTargetRef>

        assertEquals(listOf("Outer.Inner"), targets.map { it.internalName })
    }

    @Test
    fun collectorRetainsResolvedHandlersInDeclarationOrderAndAppliesUnavailableRules() {
        val firstRange = McTextRange(McTextPosition(1, 4), McTextPosition(1, 24))
        val emptyRange = McTextRange(McTextPosition(2, 4), McTextPosition(2, 24))
        val brokenRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 24))
        val secondRange = McTextRange(McTextPosition(4, 4), McTextPosition(4, 24))
        val ignoredRange = McTextRange(McTextPosition(5, 4), McTextPosition(5, 24))

        val methods = listOf(
            method(
                name = "firstHandler",
                handlerAnnotation(),
                SingleMemberAnnotation(
                    fqn = EXPRESSION_FQN,
                    typeName = "Expression",
                    value = StringLiteral("first"),
                ),
            ),
            method(name = "emptyHandler", handlerAnnotation()),
            method(
                name = "brokenHandler",
                NormalAnnotation(
                    fqn = null,
                    typeName = "ModifyExpressionValue",
                    members = emptyMap(),
                ),
            ),
            method(name = "secondHandler", handlerAnnotation()),
            method(
                name = "plainMethod",
                NormalAnnotation(
                    fqn = "org.spongepowered.asm.mixin.Shadow",
                    typeName = "Shadow",
                    members = emptyMap(),
                ),
            ),
        )

        val rangesByName = mapOf(
            "firstHandler" to firstRange,
            "emptyHandler" to emptyRange,
            "brokenHandler" to brokenRange,
            "secondHandler" to secondRange,
            "plainMethod" to ignoredRange,
        )

        val collected = JdtMixinExtrasResolvedContextCollector.collect(
            methodDeclarations = methods,
            handlerRange = { node -> rangesByName[(node as FakeMethodDeclaration).name]!! },
            methodName = { (it as FakeMethodDeclaration).name },
        )

        assertEquals(
            listOf(
                ResolvedMixinExtrasContext(
                    handlerRange = firstRange,
                    context = ExpressionContext(
                        expressionIndex = MixinExtrasExpressionIndex(
                            listOf(MixinExtrasExpression(values = listOf("first"))),
                        ),
                        definitionIndex = MixinExtrasDefinitionIndex(),
                    ),
                ),
                ResolvedMixinExtrasContext(
                    handlerRange = emptyRange,
                    context = ExpressionContext(
                        expressionIndex = MixinExtrasExpressionIndex(),
                        definitionIndex = MixinExtrasDefinitionIndex(),
                    ),
                ),
                ResolvedMixinExtrasContext(
                    handlerRange = secondRange,
                    context = ExpressionContext(
                        expressionIndex = MixinExtrasExpressionIndex(),
                        definitionIndex = MixinExtrasDefinitionIndex(),
                    ),
                ),
            ),
            collected.contexts,
        )
        assertEquals(
            listOf(
                "MixinExtras context unavailable for method brokenHandler: unresolved handler annotation binding",
            ),
            collected.warnings,
        )
    }

    @Test
    fun collectorKeepsMixedMixinExtrasWrapWithConditionAndStandardHandlers() {
        val mixinRange = McTextRange(McTextPosition(1, 4), McTextPosition(1, 24))
        val conditionRange = McTextRange(McTextPosition(2, 4), McTextPosition(2, 24))
        val standardRange = McTextRange(McTextPosition(3, 4), McTextPosition(3, 24))
        val methods = listOf(
            method(
                name = "mixinHandler",
                handlerAnnotation(),
                SingleMemberAnnotation(
                    fqn = EXPRESSION_FQN,
                    typeName = "Expression",
                    value = StringLiteral("mixinValue"),
                ),
            ),
            method(
                name = "conditionHandler",
                NormalAnnotation(
                    fqn = "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
                    typeName = "WrapWithCondition",
                    members = mapOf("method" to StringLiteral("draw()V")),
                ),
                SingleMemberAnnotation(
                    fqn = EXPRESSION_FQN,
                    typeName = "Expression",
                    value = StringLiteral("conditionValue"),
                ),
            ),
            method(
                name = "standardHandler",
                NormalAnnotation(
                    fqn = "org.spongepowered.asm.mixin.injection.Inject",
                    typeName = "Inject",
                    members = mapOf("method" to StringLiteral("draw()V")),
                ),
                SingleMemberAnnotation(
                    fqn = EXPRESSION_FQN,
                    typeName = "Expression",
                    value = StringLiteral("standardValue"),
                ),
            ),
        )
        val rangesByName = mapOf(
            "mixinHandler" to mixinRange,
            "conditionHandler" to conditionRange,
            "standardHandler" to standardRange,
        )

        val collected = JdtMixinExtrasResolvedContextCollector.collect(
            methodDeclarations = methods,
            handlerRange = { node -> rangesByName[(node as FakeMethodDeclaration).name]!! },
            methodName = { (it as FakeMethodDeclaration).name },
        )

        assertEquals(
            listOf(mixinRange, conditionRange, standardRange),
            collected.contexts.map { it.handlerRange },
        )
        assertEquals(
            listOf("mixinValue", "conditionValue", "standardValue"),
            collected.contexts.map { context ->
                context.context.expressionIndex.expressions.single().values.single()
            },
        )
        assertTrue(collected.warnings.isEmpty())
    }

    private fun method(name: String, vararg modifiers: Any): FakeMethodDeclaration =
        FakeMethodDeclaration(name, modifiers.toList())

    private fun handlerAnnotation(): NormalAnnotation =
        NormalAnnotation(
            fqn = "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
            typeName = "ModifyExpressionValue",
            members = mapOf("method" to StringLiteral("draw()V")),
        )

    private class FakeMethodDeclaration(
        val name: String,
        private val modifierList: List<Any>,
    ) {
        fun modifiers(): List<Any> = modifierList

        fun getName(): SimpleName = SimpleName(name)
    }

    private open class DomNode(
        private val recovered: Boolean = false,
        private val malformed: Boolean = false,
    ) {
        fun isRecovered(): Boolean = recovered

        fun isMalformed(): Boolean = malformed
    }

    private class TypeBinding(
        private val qualifiedName: String?,
        private val recovered: Boolean = false,
    ) {
        fun isRecovered(): Boolean = recovered
        fun getQualifiedName(): String? = qualifiedName

        fun getBinaryName(): String? = qualifiedName
    }

    private class TypeLiteral(
        private val binding: TypeBinding,
        private val sourceName: String = "Gui",
    ) : DomNode() {
        fun getType(): BoundType = BoundType(binding)
        override fun toString(): String = "$sourceName.class"
    }

    private class BoundType(private val binding: TypeBinding) {
        fun resolveBinding(): TypeBinding = binding
    }

    private class AnnotationBinding(private val typeBinding: TypeBinding?) {
        fun getAnnotationType(): TypeBinding? = typeBinding
    }

    private open class AnnotationNode(
        private val fqn: String?,
        private val typeName: String? = fqn?.substringAfterLast('.'),
        recovered: Boolean = false,
        malformed: Boolean = false,
    ) : DomNode(recovered, malformed) {
        fun resolveAnnotationBinding(): AnnotationBinding? =
            fqn?.let { AnnotationBinding(TypeBinding(it)) }

        fun getTypeName(): SimpleName = SimpleName(typeName ?: "Unknown")
    }

    private class SingleMemberAnnotation(
        fqn: String?,
        private val value: Any,
        typeName: String? = fqn?.substringAfterLast('.'),
    ) : AnnotationNode(fqn, typeName) {
        fun getValue(): Any = value
    }

    private class NormalAnnotation(
        fqn: String?,
        private val members: Map<String, Any>,
        typeName: String? = fqn?.substringAfterLast('.'),
        recovered: Boolean = false,
    ) : AnnotationNode(fqn, typeName, recovered) {
        fun values(): List<Any> =
            members.map { (name, value) -> MemberValuePair(name, value) }
    }

    private class MemberValuePair(
        private val name: String,
        private val value: Any,
    ) {
        fun getName(): SimpleName = SimpleName(name)

        fun getValue(): Any = value
    }

    private class SimpleName(
        private val identifier: String,
    ) {
        fun getIdentifier(): String = identifier

        override fun toString(): String = identifier
    }

    private class StringLiteral(
        private val constant: String?,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): String? = constant

        fun getStartPosition(): Int = 0

        fun getLength(): Int = constant?.length ?: 0
    }

    private companion object {
        private const val EXPRESSION_FQN = "com.llamalad7.mixinextras.expression.Expression"
    }
}
