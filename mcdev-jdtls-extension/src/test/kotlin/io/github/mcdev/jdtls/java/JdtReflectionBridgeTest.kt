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
        val stringOverload = FakeMethod("process", arrayOf("Ljava/lang/String;"))

        val selected = JdtReflectionBridge.selectMethod(
            arrayOf(intOverload, stringOverload),
            "process",
            "(Ljava/lang/String;)V",
            TestSignature::class.java,
        )

        assertSame(stringOverload, selected)
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
                    when (val marker = paramsSection[index]) {
                        'I', 'Z', 'B', 'C', 'S', 'J', 'F', 'D' -> {
                            params.add(marker.toString())
                            index++
                        }
                        'L' -> {
                            val end = paramsSection.indexOf(';', index)
                            require(end >= 0) { "unsupported descriptor: $descriptor" }
                            params.add(paramsSection.substring(index, end + 1))
                            index = end + 1
                        }
                        else -> error("unsupported descriptor: $descriptor")
                    }
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
