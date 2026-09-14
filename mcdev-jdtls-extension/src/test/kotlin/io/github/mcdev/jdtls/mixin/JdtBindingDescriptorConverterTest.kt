package io.github.mcdev.jdtls.mixin

import kotlin.test.Test
import kotlin.test.assertEquals

class JdtBindingDescriptorConverterTest {
    @Test
    fun recoveredTypesAndTheirArraysHaveNoDescriptor() {
        val recovered = RecoveredBinding()
        kotlin.test.assertNull(JdtBindingDescriptorConverter.descriptorFromBinding(recovered))
        kotlin.test.assertNull(JdtBindingDescriptorConverter.descriptorFromBinding(MultiDimArrayBinding(recovered, 2)))
        kotlin.test.assertNull(JdtBindingDescriptorConverter.descriptorFromBinding(ComponentOnlyArrayBinding(recovered)))
    }

    private class RecoveredBinding {
        fun isRecovered(): Boolean = true
        fun getBinaryName(): String = "Gui"
    }

    @Test
    fun descriptorFromBindingUsesElementTypeAndDimensionsForMultidimensionalPrimitiveArrays() {
        assertEquals(
            "[[I",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                MultiDimArrayBinding(PrimitiveBinding("int"), totalDims = 2),
            ),
        )
        assertEquals(
            "[[[I",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                MultiDimArrayBinding(PrimitiveBinding("int"), totalDims = 3),
            ),
        )
    }

    @Test
    fun descriptorFromBindingUsesElementTypeAndDimensionsForMultidimensionalObjectArrays() {
        assertEquals(
            "[[Ljava/lang/String;",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                MultiDimArrayBinding(ClassBinding("java.lang.String"), totalDims = 2),
            ),
        )
        assertEquals(
            "[[[Ljava/lang/String;",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                MultiDimArrayBinding(ClassBinding("java.lang.String"), totalDims = 3),
            ),
        )
    }

    @Test
    fun descriptorFromBindingAddsOneDimensionPerComponentWhenElementTypeIsAbsent() {
        assertEquals(
            "[[I",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                ComponentOnlyArrayBinding(
                    ComponentOnlyArrayBinding(PrimitiveBinding("int")),
                ),
            ),
        )
        assertEquals(
            "[[[Ljava/lang/String;",
            JdtBindingDescriptorConverter.descriptorFromBinding(
                ComponentOnlyArrayBinding(
                    ComponentOnlyArrayBinding(
                        ComponentOnlyArrayBinding(ClassBinding("java.lang.String")),
                    ),
                ),
            ),
        )
    }

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

    private class MultiDimArrayBinding(
        private val element: Any,
        private val totalDims: Int,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = true

        fun getElementType(): Any = element

        fun getDimensions(): Int = totalDims

        fun getComponentType(): Any =
            if (totalDims <= 1) {
                element
            } else {
                MultiDimArrayBinding(element, totalDims - 1)
            }
    }

    private class ComponentOnlyArrayBinding(
        private val component: Any,
    ) {
        fun isPrimitive(): Boolean = false

        fun isArray(): Boolean = true

        fun getComponentType(): Any = component
    }
}
