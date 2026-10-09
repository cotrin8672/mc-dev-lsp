package io.github.mcdev.jdtls.java

import kotlin.test.Test
import kotlin.test.assertNull
import kotlin.test.assertSame

class JdtReflectionBridgeTest {
    @Test
    fun absentDescriptorReturnsFirstSameNameOverload() {
        val intOverload = FakeMethod("process", arrayOf("I"))
        val stringOverload = FakeMethod("process", arrayOf("Ljava/lang/String;"))
        val methods = arrayOf(intOverload, stringOverload)

        val selected = JdtReflectionBridge.selectMethod(methods, "process", null, TestSignature::class.java)

        assertSame(intOverload, selected)
    }

    @Test
    fun blankDescriptorReturnsFirstSameNameOverload() {
        val first = FakeMethod("process", arrayOf("I"))
        val second = FakeMethod("process", arrayOf("Ljava/lang/String;"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(first, second),
            "process",
            "   ",
            TestSignature::class.java,
        )

        assertSame(first, selected)
    }

    @Test
    fun explicitDescriptorExactMatchSelectsMatchingOverload() {
        val intOverload = FakeMethod("process", arrayOf("I"))
        val stringOverload = FakeMethod("process", arrayOf("Ljava.lang.String;"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(intOverload, stringOverload),
            "process",
            "(Ljava/lang/String;)V",
            TestSignature::class.java,
        )

        assertSame(stringOverload, selected)
    }

    @Test
    fun explicitDescriptorMatchesPrimitiveArrayAndErasedGenericOverloads() {
        val genericOverload = FakeMethod("process", arrayOf("Ljava.util.List<Ljava.lang.String;>;"))
        val primitiveOverload = FakeMethod("process", arrayOf("I"))
        val arrayOverload = FakeMethod("process", arrayOf("[Lnet.minecraft.world.item.ItemStack;"))
        val methods = arrayOf(genericOverload, primitiveOverload, arrayOverload)

        val selectedArray = JdtReflectionBridge.selectMethod(
            methods,
            "process",
            "([Lnet/minecraft/world/item/ItemStack;)V",
            TestSignature::class.java,
        )
        val selectedPrimitive = JdtReflectionBridge.selectMethod(
            methods,
            "process",
            "(I)V",
            TestSignature::class.java,
        )
        val selectedGeneric = JdtReflectionBridge.selectMethod(
            methods,
            "process",
            "(Ljava/util/List;)V",
            TestSignature::class.java,
        )

        assertSame(arrayOverload, selectedArray)
        assertSame(primitiveOverload, selectedPrimitive)
        assertSame(genericOverload, selectedGeneric)
    }

    @Test
    fun explicitDescriptorWithAmbiguousNormalizedMatchesReturnsNull() {
        val rawOverload = FakeMethod("process", arrayOf("Ljava.util.List;"))
        val genericOverload = FakeMethod("process", arrayOf("Ljava.util.List<Ljava.lang.String;>;"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(genericOverload, rawOverload),
            "process",
            "(Ljava/util/List;)V",
            TestSignature::class.java,
        )

        assertNull(selected)
    }

    @Test
    fun onlyQualifiedUnresolvedTypesAreNormalized() {
        val qualified = FakeMethod("process", arrayOf("Qnet.minecraft.ItemStack;"))
        val simple = FakeMethod("process", arrayOf("QString;"))

        val qualifiedSelected = JdtReflectionBridge.selectMethod(
            arrayOf(qualified),
            "process",
            "(Lnet/minecraft/ItemStack;)V",
            TestSignature::class.java,
        )
        val simpleSelected = JdtReflectionBridge.selectMethod(
            arrayOf(simple),
            "process",
            "(Ljava/lang/String;)V",
            TestSignature::class.java,
        )

        assertSame(qualified, qualifiedSelected)
        assertNull(simpleSelected)
    }

    @Test
    fun explicitDescriptorMismatchReturnsNull() {
        val intOverload = FakeMethod("process", arrayOf("I"))
        val stringOverload = FakeMethod("process", arrayOf("Ljava/lang/String;"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(intOverload, stringOverload),
            "process",
            "(D)V",
            TestSignature::class.java,
        )

        assertNull(selected)
    }

    @Test
    fun explicitDescriptorWhenParameterExtractionFailsReturnsNull() {
        val method = FakeMethod("process", arrayOf("I"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(method),
            "process",
            "(I)V",
            FailingTestSignature::class.java,
        )

        assertNull(selected)
    }

    @Test
    fun explicitDescriptorWhenSignatureUnavailableReturnsNull() {
        val method = FakeMethod("process", arrayOf("I"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(method),
            "process",
            "(I)V",
            signatureClass = null,
        )

        assertNull(selected)
    }

    @Test
    fun explicitDescriptorWhenMethodParameterExtractionFailsReturnsNull() {
        val method = MissingParameterTypesMethod("process")

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(method),
            "process",
            "(I)V",
            TestSignature::class.java,
        )

        assertNull(selected)
    }

    private class FakeMethod(
        private val elementName: String,
        private val parameterTypes: Array<String>,
    ) {
        fun getElementName(): String = elementName

        fun getParameterTypes(): Array<String> = parameterTypes
    }

    private class MissingParameterTypesMethod(
        private val elementName: String,
    ) {
        fun getElementName(): String = elementName
    }

    private class TestSignature {
        companion object {
            @JvmStatic
            fun getTypeErasure(signature: String): String {
                val start = signature.indexOf('<')
                if (start < 0) return signature
                val end = signature.lastIndexOf('>')
                require(end > start) { "unsupported signature: $signature" }
                return signature.removeRange(start, end + 1)
            }

            @JvmStatic
            fun getParameterTypes(descriptor: String): Array<String> {
                require(descriptor.startsWith('(') && descriptor.contains(')')) {
                    "unsupported descriptor: $descriptor"
                }
                val paramsSection = descriptor.substring(1, descriptor.indexOf(')'))
                if (paramsSection.isEmpty()) {
                    return emptyArray()
                }
                val params = mutableListOf<String>()
                var index = 0
                while (index < paramsSection.length) {
                    val start = index
                    while (index < paramsSection.length && paramsSection[index] == '[') {
                        index++
                    }
                    require(index < paramsSection.length) { "unsupported descriptor: $descriptor" }
                    when (val marker = paramsSection[index]) {
                        'I', 'Z', 'B', 'C', 'S', 'J', 'F', 'D' -> {
                            index++
                        }
                        'L' -> {
                            val end = paramsSection.indexOf(';', index)
                            require(end >= 0) { "unsupported descriptor: $descriptor" }
                            index = end + 1
                        }
                        else -> error("unsupported descriptor: $descriptor")
                    }
                    params.add(paramsSection.substring(start, index))
                }
                return params.toTypedArray()
            }
        }
    }

    private class FailingTestSignature {
        companion object {
            @JvmStatic
            fun getParameterTypes(descriptor: String): Array<String> {
                error("parameter extraction failed for $descriptor")
            }
        }
    }
}
