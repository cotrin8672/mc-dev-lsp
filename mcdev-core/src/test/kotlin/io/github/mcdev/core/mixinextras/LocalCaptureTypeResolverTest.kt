package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeClassIndex
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class LocalCaptureTypeResolverTest {
    private val classIndex = FakeClassIndex(
        classes = FakeClassIndex.defaultClasses() + listOf(
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
            ClassIndexEntry("List", "java.util", "java/util/List"),
            ClassIndexEntry("LocalRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalRef"),
            ClassIndexEntry("LocalBooleanRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalBooleanRef"),
            ClassIndexEntry("LocalByteRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalByteRef"),
            ClassIndexEntry("LocalCharRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalCharRef"),
            ClassIndexEntry("LocalShortRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalShortRef"),
            ClassIndexEntry("LocalIntRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalIntRef"),
            ClassIndexEntry("LocalLongRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalLongRef"),
            ClassIndexEntry("LocalFloatRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalFloatRef"),
            ClassIndexEntry("LocalDoubleRef", "com.llamalad7.mixinextras.sugar.ref", "com/llamalad7/mixinextras/sugar/ref/LocalDoubleRef"),
            ClassIndexEntry(
                "NestedLocalRef",
                "com.llamalad7.mixinextras.sugar.ref.nested",
                "com/llamalad7/mixinextras/sugar/ref/nested/NestedLocalRef",
            ),
            ClassIndexEntry("LocalRef", "com.example.lookalike", "com/example/lookalike/LocalRef"),
            ClassIndexEntry("LocalIntRef", "com.example.lookalike", "com/example/lookalike/LocalIntRef"),
        ),
    )
    private val resolver = LocalCaptureTypeResolver(classIndex)

    private val source = """
        package com.example.mixin;

        import com.llamalad7.mixinextras.sugar.ref.*;
        import java.util.List;

        abstract class ExampleMixin {
        }
    """.trimIndent()

    private fun parameter(typeName: String, sugarSpec: HandlerParameterSugarSpec.Local? = null) =
        HandlerParameterDeclaration(
            name = "value",
            typeName = typeName,
            typeDescriptor = null,
            isOperation = false,
            operationGenericName = null,
            isSugar = sugarSpec != null,
            sugarSpec = sugarSpec,
        )

    @Test
    fun resolveParameterResolvesPlainPrimitiveAndReferenceTypes() {
        assertEquals("I", resolver.resolveParameter(source, parameter("int")))
        assertEquals("J", resolver.resolveParameter(source, parameter("long")))
        assertEquals(
            "Ljava/lang/String;",
            resolver.resolveParameter(source, parameter("String")),
        )
    }

    @Test
    fun resolveParameterUnwrapsOfficialPrimitiveRefTypes() {
        assertEquals("Z", resolver.resolveParameter(source, parameter("LocalBooleanRef")))
        assertEquals("B", resolver.resolveParameter(source, parameter("LocalByteRef")))
        assertEquals("C", resolver.resolveParameter(source, parameter("LocalCharRef")))
        assertEquals("S", resolver.resolveParameter(source, parameter("LocalShortRef")))
        assertEquals("I", resolver.resolveParameter(source, parameter("LocalIntRef")))
        assertEquals("J", resolver.resolveParameter(source, parameter("LocalLongRef")))
        assertEquals("F", resolver.resolveParameter(source, parameter("LocalFloatRef")))
        assertEquals("D", resolver.resolveParameter(source, parameter("LocalDoubleRef")))
    }

    @Test
    fun resolveParameterUnwrapsOfficialLocalRefWithConcreteGeneric() {
        assertEquals(
            "Ljava/lang/String;",
            resolver.resolveParameter(source, parameter("LocalRef<String>")),
        )
        assertEquals(
            "Ljava/lang/String;",
            resolver.resolveParameter(
                source,
                parameter("com.llamalad7.mixinextras.sugar.ref.LocalRef<String>"),
            ),
        )
    }

    @Test
    fun resolveParameterUnwrapsOfficialLocalRefReferenceArrayGeneric() {
        assertEquals(
            "[Ljava/lang/String;",
            resolver.resolveParameter(source, parameter("LocalRef<String[]>")),
        )
    }

    @Test
    fun resolveParameterUnwrapsOfficialLocalRefErasedGenericType() {
        assertEquals(
            "Ljava/util/List;",
            resolver.resolveParameter(source, parameter("LocalRef<List<String>>")),
        )
    }

    @Test
    fun resolveParameterReturnsNullForRawWildcardMultipleOrUnresolvedLocalRefGenerics() {
        assertNull(resolver.resolveParameter(source, parameter("LocalRef")))
        assertNull(resolver.resolveParameter(source, parameter("LocalRef<?>")))
        assertNull(resolver.resolveParameter(source, parameter("LocalRef<? extends String>")))
        assertNull(resolver.resolveParameter(source, parameter("LocalRef<String, Integer>")))
        assertNull(resolver.resolveParameter(source, parameter("LocalRef<MissingType>")))
        assertNull(resolver.resolveParameter(source, parameter("LocalRef<String>[]")))
    }

    @Test
    fun resolveParameterReturnsNullForWrongPackageLocalRefLookalike() {
        assertNull(
            resolver.resolveParameter(
                source,
                parameter("com.example.lookalike.LocalRef<String>"),
            ),
        )
    }

    @Test
    fun resolveParameterLeavesWrongPackagePrimitiveRefDescriptorUnwrapped() {
        assertEquals(
            "Lcom/example/lookalike/LocalIntRef;",
            resolver.resolveParameter(
                source,
                parameter("com.example.lookalike.LocalIntRef"),
            ),
        )
    }

    @Test
    fun resolveParameterLeavesOtherOfficialSugarRefTypesUnchanged() {
        assertEquals(
            "Lcom/llamalad7/mixinextras/sugar/ref/nested/NestedLocalRef;",
            resolver.resolveParameter(
                source,
                parameter("com.llamalad7.mixinextras.sugar.ref.nested.NestedLocalRef"),
            ),
        )
    }

    @Test
    fun resolveParameterIgnoresLocalDefinitionOnlyTypeAttribute() {
        val param = parameter(
            typeName = "int",
            sugarSpec = HandlerParameterSugarSpec.Local(typeClassName = "java.lang.String"),
        )
        assertEquals("I", resolver.resolveParameter(source, param))
    }

    @Test
    fun resolveParameterReturnsNullForUnresolvedType() {
        assertNull(resolver.resolveParameter(source, parameter("MissingType")))
    }

    @Test
    fun resolveDefinitionResolvesNonVoidTypeClassName() {
        assertEquals(
            "Ljava/lang/String;",
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "String"),
            ),
        )
        assertEquals(
            "Ljava/lang/String;",
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "java.lang.String"),
            ),
        )
        assertEquals(
            "I",
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "int"),
            ),
        )
        assertEquals(
            "[Ljava/lang/String;",
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "java.lang.String[]"),
            ),
        )
    }

    @Test
    fun resolveDefinitionReturnsNullForMissingOrVoidTypeClassName() {
        assertNull(resolver.resolveDefinition(source, HandlerParameterSugarSpec.Local()))
        assertNull(
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "void"),
            ),
        )
    }

    @Test
    fun resolveDefinitionReturnsNullForUnresolvedTypeClassName() {
        assertNull(
            resolver.resolveDefinition(
                source,
                HandlerParameterSugarSpec.Local(typeClassName = "MissingType"),
            ),
        )
    }
}
