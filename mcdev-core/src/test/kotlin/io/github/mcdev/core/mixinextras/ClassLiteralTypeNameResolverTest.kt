package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FakeClassIndex
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.JavaTypeResolutionContext
import io.github.mcdev.core.mixin.TypeDescriptorResult
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertIs

class ClassLiteralTypeNameResolverTest {
    @Test
    fun conflictingExplicitImportsRemainAmbiguousWhileIdenticalImportsAreAllowed() {
        val source = "import one.Foo;\nimport two.Foo;\nimport one.Foo;"
        val classes = arrayOf(ClassIndexEntry("Foo", "one", "one/Foo"),
            ClassIndexEntry("Foo", "two", "two/Foo"))
        assertNull(resolver(source, *classes).resolve("Foo"))
        assertNull(resolver(source, *classes).resolve("Foo.Inner"))
        assertIs<TypeDescriptorResult.Ambiguous>(JavaTypeDescriptorResolver.descriptorOrDiagnostic(
            "Foo", JavaTypeResolutionContext(JavaTypeDescriptorResolver.importsFor(source))))
        assertEquals("one/Foo", resolver("import one.Foo;\nimport one.Foo;", *classes)
            .resolve("Foo")?.internalName)
    }

    @Test
    fun singleLetterJavaClassNamesAreNotPrimitiveDescriptors() {
        val resolver = resolver("import sample.B;", ClassIndexEntry("B", "sample", "sample/B"))
        assertEquals("Lsample/B;", resolver.resolve("B")?.descriptor)
        assertEquals("[Lsample/B;", resolver.resolve("B[]")?.descriptor)
        assertEquals("I", resolver.resolve("int")?.descriptor)
        assertNull(resolver.resolve("I"))
    }

    @Test
    fun resolvesIndexedImplicitJavaLangType() {
        val resolver = resolver(
            source = "@Definition(local = @Local(type = String.class))",
            ClassIndexEntry("String", "java.lang", "java/lang/String"),
        )

        assertEquals("java/lang/String", resolver.resolve("String")?.internalName)
    }

    @Test
    fun doesNotGuessUnindexedImplicitJavaLangType() {
        val resolver = resolver("@Definition(local = @Local(type = String.class))")

        assertNull(resolver.resolve("String"))
    }

    @Test
    fun resolvesExplicitImport() {
        val resolver = resolver(
            source = """
                import com.example.Foo;
                @Definition(local = @Local(type = Foo.class))
            """.trimIndent(),
            ClassIndexEntry("Foo", "com.example", "com/example/Foo"),
        )

        assertEquals("com/example/Foo", resolver.resolve("Foo")?.internalName)
    }

    @Test
    fun resolvesSamePackageType() {
        val resolver = resolver(
            source = """
                package com.example;
                @Definition(local = @Local(type = Foo.class))
            """.trimIndent(),
            ClassIndexEntry("Foo", "com.example", "com/example/Foo"),
        )

        assertEquals("com/example/Foo", resolver.resolve("Foo")?.internalName)
    }

    @Test
    fun resolvesNestedTypeThroughImportedOuterType() {
        val resolver = resolver(
            source = """
                import com.example.Outer;
                @Definition(local = @Local(type = Outer.Inner.class))
            """.trimIndent(),
            ClassIndexEntry("Outer\$Inner", "com.example", "com/example/Outer\$Inner"),
        )

        assertEquals("com/example/Outer\$Inner", resolver.resolve("Outer.Inner")?.internalName)
    }

    @Test
    fun resolvesNestedTypeThroughSamePackage() {
        val resolver = resolver(
            source = """
                package com.example;
                @Definition(local = @Local(type = Outer.Inner.class))
            """.trimIndent(),
            ClassIndexEntry("Outer\$Inner", "com.example", "com/example/Outer\$Inner"),
        )

        assertEquals("com/example/Outer\$Inner", resolver.resolve("Outer.Inner")?.internalName)
    }

    @Test
    fun rejectsAmbiguousWildcardAndImplicitJavaLangType() {
        val resolver = resolver(
            source = """
                import one.*;
                import two.*;
                @Definition(local = @Local(type = Foo.class))
            """.trimIndent(),
            ClassIndexEntry("Foo", "one", "one/Foo"),
            ClassIndexEntry("Foo", "two", "two/Foo"),
            ClassIndexEntry("Foo", "java.lang", "java/lang/Foo"),
        )

        assertNull(resolver.resolve("Foo"))
    }

    private fun resolver(source: String, vararg classes: ClassIndexEntry): ClassLiteralTypeNameResolver =
        ClassLiteralTypeNameResolver.forSource(source, FakeClassIndex(classes.toList()))
}
