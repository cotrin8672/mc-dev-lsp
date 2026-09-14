package io.github.mcdev.jdtls.mixin

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class JdtTypeSignatureConverterTest {
    private val sourceTypeResolver: UnresolvedErasureNameResolver = { erasureName ->
        when (erasureName) {
            "String" -> "java/lang/String"
            "java.util.List" -> "java/util/List"
            "Outer.Inner" -> "com/example/Outer\$Inner"
            else -> null
        }
    }

    @Test
    fun convertsPrimitiveInt() {
        assertEquals("I", JdtTypeSignatureConverter.toJvmDescriptorOrNull("I"))
    }

    @Test
    fun convertsIntArrayOfArrays() {
        assertEquals("[[I", JdtTypeSignatureConverter.toJvmDescriptorOrNull("[[I"))
    }

    @Test
    fun convertsUnresolvedStringViaResolver() {
        assertEquals(
            "Ljava/lang/String;",
            JdtTypeSignatureConverter.toJvmDescriptorOrNull("QString;", sourceTypeResolver),
        )
    }

    @Test
    fun erasesParameterizedUnresolvedList() {
        assertEquals(
            "Ljava/util/List;",
            JdtTypeSignatureConverter.toJvmDescriptorOrNull(
                "Qjava.util.List<Ljava.lang.String;>;",
                sourceTypeResolver,
            ),
        )
    }

    @Test
    fun convertsResolvedJavaLangString() {
        assertEquals(
            "Ljava/lang/String;",
            JdtTypeSignatureConverter.toJvmDescriptorOrNull("Ljava.lang.String;"),
        )
    }

    @Test
    fun convertsUnresolvedInnerClassViaResolver() {
        assertEquals(
            "Lcom/example/Outer\$Inner;",
            JdtTypeSignatureConverter.toJvmDescriptorOrNull("QOuter.Inner;", sourceTypeResolver),
        )
    }

    @Test
    fun erasesTypeVariableToObject() {
        assertEquals(
            "Ljava/lang/Object;",
            JdtTypeSignatureConverter.toJvmDescriptorOrNull("TT;"),
        )
    }

    @Test
    fun returnsNullForUnknownUnresolvedClass() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull("QUnknown.Type;", sourceTypeResolver))
    }

    @Test
    fun returnsNullForUnknownUnresolvedClassWithoutResolver() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull("QString;"))
    }

    @Test
    fun returnsNullForArrayOfUnknownUnresolvedClass() {
        assertNull(
            JdtTypeSignatureConverter.toJvmDescriptorOrNull("[QMissing.Type;", sourceTypeResolver),
        )
    }

    @Test
    fun returnsNullForEmptySignature() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull(""))
    }

    @Test
    fun returnsNullForUnsupportedLeadingCharacter() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull("@I"))
    }

    @Test
    fun returnsNullForMissingClassTerminator() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull("Ljava.lang.String"))
    }

    @Test
    fun returnsNullForUnbalancedTypeArguments() {
        assertNull(JdtTypeSignatureConverter.toJvmDescriptorOrNull("Ljava.util.List<Ljava.lang.String;"))
    }
}
