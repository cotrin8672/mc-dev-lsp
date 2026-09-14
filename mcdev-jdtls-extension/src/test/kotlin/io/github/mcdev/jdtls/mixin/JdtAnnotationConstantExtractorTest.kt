package io.github.mcdev.jdtls.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertFailsWith
import kotlin.test.assertSame
import java.util.concurrent.CancellationException

class JdtAnnotationConstantExtractorTest {
    @Test
    fun cancelledConstantResolutionPropagatesAndCanRecover() {
        val cancellation = CancellationException("constant resolution cancelled")
        val node = object {
            var cancelled = true
            fun resolveConstantExpressionValue(): String {
                if (cancelled) throw cancellation
                return "resolved"
            }
        }
        assertSame(cancellation, assertFailsWith<CancellationException> {
            JdtAnnotationConstantExtractor.constantString(node)
        })
        node.cancelled = false
        assertEquals("resolved", JdtAnnotationConstantExtractor.constantString(node))
    }

    @Test
    fun constantStringAcceptsResolvedScalarValue() {
        assertEquals("hello", JdtAnnotationConstantExtractor.constantString(StringLiteral("hello")))
    }

    @Test
    fun constantStringAcceptsEscapedAndConcatenatedResolverValues() {
        assertEquals("a\tb", JdtAnnotationConstantExtractor.constantString(StringLiteral("a\tb")))
        assertEquals("prefixsuffix", JdtAnnotationConstantExtractor.constantString(StringLiteral("prefixsuffix")))
    }

    @Test
    fun constantStringRejectsRecoveredAndMalformedNodesViaGetFlags() {
        assertNull(
            JdtAnnotationConstantExtractor.constantString(
                StringLiteralWithFlags("ok", flags = AstNodeFlags.RECOVERED),
            ),
        )
        assertNull(
            JdtAnnotationConstantExtractor.constantString(
                StringLiteralWithFlags("ok", flags = AstNodeFlags.MALFORMED),
            ),
        )
        assertEquals(
            "ok",
            JdtAnnotationConstantExtractor.constantString(
                StringLiteralWithFlags("ok", flags = 0),
            ),
        )
    }

    @Test
    fun constantStringRejectsNullConstantWrongTypeAndRecoveredNode() {
        assertNull(JdtAnnotationConstantExtractor.constantString(StringLiteral(null)))
        assertNull(JdtAnnotationConstantExtractor.constantString(StringLiteral(42)))
        assertNull(JdtAnnotationConstantExtractor.constantString(StringLiteral("ok", recovered = true)))
        assertNull(JdtAnnotationConstantExtractor.constantString(StringLiteral("ok", malformed = true)))
    }

    @Test
    fun constantBooleanAndIntAcceptOnlyMatchingRuntimeTypes() {
        assertEquals(true, JdtAnnotationConstantExtractor.constantBoolean(BooleanLiteral(true)))
        assertEquals(false, JdtAnnotationConstantExtractor.constantBoolean(BooleanLiteral(false)))
        assertEquals(16, JdtAnnotationConstantExtractor.constantInt(IntLiteral(16)))

        assertNull(JdtAnnotationConstantExtractor.constantBoolean(IntLiteral(1)))
        assertNull(JdtAnnotationConstantExtractor.constantInt(IntLiteral(null)))
        assertNull(JdtAnnotationConstantExtractor.constantInt(LongLiteral(16L)))
    }

    @Test
    fun stringArrayAcceptsScalarOrArrayInitializerPreservingOrder() {
        assertEquals(
            listOf("only"),
            JdtAnnotationConstantExtractor.stringArray(StringLiteral("only")),
        )
        assertEquals(
            listOf("first", "second"),
            JdtAnnotationConstantExtractor.stringArray(
                ArrayInitializer(
                    StringLiteral("first"),
                    StringLiteral("second"),
                ),
            ),
        )
        assertEquals(
            emptyList(),
            JdtAnnotationConstantExtractor.stringArray(ArrayInitializer()),
        )
    }

    @Test
    fun stringArrayRejectsUnresolvedNullOrWrongTypeElements() {
        assertNull(
            JdtAnnotationConstantExtractor.stringArray(
                ArrayInitializer(
                    StringLiteral("good"),
                    StringLiteral(null),
                ),
            ),
        )
        assertNull(
            JdtAnnotationConstantExtractor.stringArray(
                ArrayInitializer(
                    StringLiteral("good"),
                    IntLiteral(1),
                ),
            ),
        )
        assertNull(
            JdtAnnotationConstantExtractor.stringArray(
                ArrayInitializer(
                    StringLiteral("good"),
                    null,
                ),
            ),
        )
    }

    @Test
    fun typeLiteralDescriptorUsesBindingConversionForPrimitiveObjectAndArrayTypes() {
        assertEquals("I", JdtAnnotationConstantExtractor.typeLiteralDescriptor(TypeLiteral(PrimitiveBinding("int"))))
        assertEquals(
            "Ljava/lang/String;",
            JdtAnnotationConstantExtractor.typeLiteralDescriptor(TypeLiteral(ClassBinding("java.lang.String"))),
        )
        assertEquals(
            "[Ljava/lang/String;",
            JdtAnnotationConstantExtractor.typeLiteralDescriptor(
                TypeLiteral(ArrayBinding(ClassBinding("java.lang.String"), dims = 1)),
            ),
        )
    }

    @Test
    fun typeLiteralSourceNameConvertsEveryPrimitiveAndPreservesObjectDescriptors() {
        val primitiveNames = listOf(
            "void",
            "boolean",
            "byte",
            "char",
            "short",
            "int",
            "long",
            "float",
            "double",
        )

        assertEquals(
            primitiveNames,
            primitiveNames.map { name ->
                JdtAnnotationConstantExtractor.typeLiteralSourceName(TypeLiteral(PrimitiveBinding(name)))
            },
        )
        assertEquals(
            "Ljava/lang/String;",
            JdtAnnotationConstantExtractor.typeLiteralSourceName(
                TypeLiteral(ClassBinding("java.lang.String")),
            ),
        )
        assertEquals(
            "[Ljava/lang/String;",
            JdtAnnotationConstantExtractor.typeLiteralSourceName(
                TypeLiteral(ArrayBinding(ClassBinding("java.lang.String"), dims = 1)),
            ),
        )
    }

    @Test
    fun typeLiteralDescriptorUsesReferencedTypeInsteadOfClassExpressionBinding() {
        assertEquals(
            "Lcom/example/SimpleTarget;",
            JdtAnnotationConstantExtractor.typeLiteralDescriptor(
                TypeLiteral(ClassBinding("com.example.SimpleTarget")),
            ),
        )
    }

    @Test
    fun typeLiteralDescriptorRejectsNullBindingAndRecoveredNode() {
        assertNull(JdtAnnotationConstantExtractor.typeLiteralDescriptor(TypeLiteral(null)))
        assertNull(JdtAnnotationConstantExtractor.typeLiteralDescriptor(TypeLiteral(PrimitiveBinding("int"), recovered = true)))
    }

    @Test
    fun typeArrayAcceptsScalarOrArrayInitializerPreservingOrder() {
        assertEquals(
            listOf("Ljava/lang/String;"),
            JdtAnnotationConstantExtractor.typeArray(TypeLiteral(ClassBinding("java.lang.String"))),
        )
        assertEquals(
            listOf("Ljava/lang/String;", "I"),
            JdtAnnotationConstantExtractor.typeArray(
                ArrayInitializer(
                    TypeLiteral(ClassBinding("java.lang.String")),
                    TypeLiteral(PrimitiveBinding("int")),
                ),
            ),
        )
    }

    @Test
    fun typeArrayRejectsUnresolvedElementForWholeArray() {
        assertNull(
            JdtAnnotationConstantExtractor.typeArray(
                ArrayInitializer(
                    TypeLiteral(ClassBinding("java.lang.String")),
                    TypeLiteral(null),
                ),
            ),
        )
    }

    @Test
    fun nestedAnnotationArrayAcceptsSingleAnnotationOrArrayInitializerPreservingOrder() {
        val first = NormalAnnotation("first")
        val second = NormalAnnotation("second")

        assertEquals(
            listOf(first),
            JdtAnnotationConstantExtractor.nestedAnnotationArray(first),
        )
        assertEquals(
            listOf(first, second),
            JdtAnnotationConstantExtractor.nestedAnnotationArray(
                ArrayInitializer(first, second),
            ),
        )
    }

    @Test
    fun nestedAnnotationArrayRejectsWrongTypeRecoveredOrNullElements() {
        val valid = NormalAnnotation("valid")

        assertNull(JdtAnnotationConstantExtractor.nestedAnnotationArray(StringLiteral("not-annotation")))
        assertNull(
            JdtAnnotationConstantExtractor.nestedAnnotationArray(
                ArrayInitializer(valid, StringLiteral("bad")),
            ),
        )
        assertNull(
            JdtAnnotationConstantExtractor.nestedAnnotationArray(
                ArrayInitializer(valid, null),
            ),
        )
        assertNull(
            JdtAnnotationConstantExtractor.nestedAnnotationArray(
                ArrayInitializer(valid, NormalAnnotation("recovered", recovered = true)),
            ),
        )
    }

    private open class AstNodeFlags {
        companion object {
            const val RECOVERED = 0x2
            const val MALFORMED = 0x4
        }
    }

    private open class DomNode(
        private val recovered: Boolean = false,
        private val malformed: Boolean = false,
    ) {
        fun isRecovered(): Boolean = recovered

        fun isMalformed(): Boolean = malformed
    }

    private class StringLiteralWithFlags(
        private val constant: Any?,
        private val flags: Int = 0,
    ) : AstNodeFlags() {
        fun getFlags(): Int = flags

        fun resolveConstantExpressionValue(): Any? = constant
    }

    private class StringLiteral(
        private val constant: Any?,
        recovered: Boolean = false,
        malformed: Boolean = false,
    ) : DomNode(recovered, malformed) {
        fun resolveConstantExpressionValue(): Any? = constant
    }

    private class BooleanLiteral(
        private val constant: Boolean,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): Any = constant
    }

    private class IntLiteral(
        private val constant: Int?,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): Any? = constant
    }

    private class LongLiteral(
        private val constant: Long,
    ) : DomNode() {
        fun resolveConstantExpressionValue(): Any = constant
    }

    private class ArrayInitializer(
        vararg expressions: Any?,
    ) : DomNode() {
        private val values: List<Any?> = expressions.toList()

        fun expressions(): List<Any?> = values
    }

    private class TypeLiteral(
        private val binding: Any?,
        recovered: Boolean = false,
    ) : DomNode(recovered) {
        fun getType(): TypeNode = TypeNode(binding)

        fun resolveTypeBinding(): Any = ClassBinding("java.lang.Class")
    }

    private class TypeNode(
        private val binding: Any?,
    ) {
        fun resolveBinding(): Any? = binding
    }

    private class NormalAnnotation(
        val label: String,
        recovered: Boolean = false,
    ) : DomNode(recovered)

    private class PrimitiveBinding(
        private val name: String,
    ) {
        fun isPrimitive(): Boolean = true

        fun isArray(): Boolean = false

        fun getName(): String = name
    }

    private class ClassBinding(
        private val binaryName: String,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = false

        fun getErasure(): Any = this

        fun getBinaryName(): String = binaryName
    }

    private class ArrayBinding(
        private val component: Any,
        private val dims: Int,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = true

        fun getComponentType(): Any = component

        fun getDimensions(): Int = dims
    }
}
