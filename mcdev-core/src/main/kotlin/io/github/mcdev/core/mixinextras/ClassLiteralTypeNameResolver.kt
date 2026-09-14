package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.JavaSourceImports
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import org.objectweb.asm.Type

fun interface ClassLiteralTypeNameResolver {
    fun resolve(typeName: String): Type?

    companion object {
        val FAIL_CLOSED: ClassLiteralTypeNameResolver = FailClosedClassLiteralTypeNameResolver

        fun forSource(source: String, classIndex: ClassIndex): ClassLiteralTypeNameResolver =
            forImports(
                imports = JavaTypeDescriptorResolver.importsFor(source),
                classIndex = classIndex,
            )

        fun forImports(
            imports: JavaSourceImports,
            classIndex: ClassIndex,
        ): ClassLiteralTypeNameResolver =
            SourceClassLiteralTypeNameResolver(
                imports = imports,
                classIndex = classIndex,
            )
    }
}

private class SourceClassLiteralTypeNameResolver(
    private val imports: JavaSourceImports,
    private val classIndex: ClassIndex,
) : ClassLiteralTypeNameResolver {
    override fun resolve(typeName: String): Type? {
        if (typeName.isBlank() || isMethodDescriptorLike(typeName)) return null

        val (baseName, arrayDimensions) = splitArraySuffix(typeName)
        val resolvedBase = resolveBase(baseName) ?: return null
        if (arrayDimensions > 0 && resolvedBase.sort == Type.VOID) return null

        var resolved = resolvedBase
        repeat(arrayDimensions) {
            resolved = Type.getType("[${resolved.descriptor}")
        }
        return resolved
    }

    private fun resolveBase(baseName: String): Type? {
        if (baseName.isBlank()) return null
        if (looksLikeJvmTypeDescriptor(baseName)) {
            return ClassLiteralTypeNameResolver.FAIL_CLOSED.resolve(baseName)
        }
        if ('/' in baseName) {
            return classIndex.findClass(baseName)?.let { Type.getObjectType(it.internalName) }
        }

        val primitive = PRIMITIVE_TYPES[baseName]
        if (primitive != null) return primitive

        val internalName = resolveSourceClass(baseName) ?: return null
        return Type.getObjectType(internalName)
    }

    private fun resolveSourceClass(typeName: String): String? {
        if (typeName.substringBefore('.') in imports.ambiguousExplicit) return null
        val explicitImport = imports.explicit[typeName]
        if (explicitImport != null) {
            return resolveQualifiedName(explicitImport)
        }

        if ('.' in typeName) {
            val firstSegment = typeName.substringBefore('.')
            val nestedImport = imports.explicit[firstSegment]
            if (nestedImport != null) {
                resolveQualifiedName("$nestedImport.${typeName.substringAfter('.')}")?.let { return it }
            }
            resolveQualifiedName(typeName)?.let { return it }
            imports.packageName?.let { packageName ->
                resolveQualifiedName("$packageName.$typeName")?.let { return it }
            }
            return resolveOnDemand(typeName)
        }

        val samePackage = imports.packageName?.let { resolveQualifiedName("$it.$typeName") }
        if (samePackage != null) return samePackage

        return resolveOnDemand(typeName)
            ?: resolveQualifiedName(typeName)
    }

    private fun resolveOnDemand(typeName: String): String? {
        val wildcardMatches = (imports.wildcardPackages + "java.lang")
            .flatMap { packageName -> resolveQualifiedNames("$packageName.$typeName") }
            .distinct()
        if (wildcardMatches.size == 1) return wildcardMatches.single()
        if (wildcardMatches.size > 1) return null

        return null
    }

    private fun resolveQualifiedName(name: String): String? =
        resolveQualifiedNames(name).singleOrNull()

    private fun resolveQualifiedNames(name: String): List<String> {
        val candidates = linkedSetOf<String>()
        classIndex.findClassByFqn(name)?.let { candidates += it.internalName }
        if ('/' in name) {
            classIndex.findClass(name)?.let { candidates += it.internalName }
        }

        val parts = name.split('.')
        for (classStart in 1 until parts.size) {
            val packageName = parts.take(classStart).joinToString(".")
            val className = parts.drop(classStart).joinToString("$")
            classIndex.findClassByFqn("$packageName.$className")?.let { candidates += it.internalName }
        }
        return candidates.toList()
    }

    private fun splitArraySuffix(typeName: String): Pair<String, Int> {
        var baseName = typeName.trim()
        var dimensions = 0
        while (baseName.endsWith("[]")) {
            baseName = baseName.dropLast(2).trim()
            dimensions++
        }
        return baseName to dimensions
    }

    private fun looksLikeJvmTypeDescriptor(name: String): Boolean =
        name.startsWith("[") ||
            (name.startsWith("L") && name.endsWith(";"))

    private companion object {
        val PRIMITIVE_TYPES = mapOf(
            "void" to Type.VOID_TYPE,
            "boolean" to Type.BOOLEAN_TYPE,
            "byte" to Type.BYTE_TYPE,
            "char" to Type.CHAR_TYPE,
            "short" to Type.SHORT_TYPE,
            "int" to Type.INT_TYPE,
            "long" to Type.LONG_TYPE,
            "float" to Type.FLOAT_TYPE,
            "double" to Type.DOUBLE_TYPE,
        )
    }
}

private object FailClosedClassLiteralTypeNameResolver : ClassLiteralTypeNameResolver {
    private val PRIMITIVE_TYPES = mapOf(
        "void" to Type.VOID_TYPE,
        "boolean" to Type.BOOLEAN_TYPE,
        "byte" to Type.BYTE_TYPE,
        "char" to Type.CHAR_TYPE,
        "short" to Type.SHORT_TYPE,
        "int" to Type.INT_TYPE,
        "long" to Type.LONG_TYPE,
        "float" to Type.FLOAT_TYPE,
        "double" to Type.DOUBLE_TYPE,
    )

    override fun resolve(typeName: String): Type? {
        if (typeName.isBlank()) {
            return null
        }
        if (isMethodDescriptorLike(typeName)) {
            return null
        }

        val (baseName, arrayDimensions) = splitArraySuffix(typeName)
        val resolvedBase = resolveBase(baseName) ?: return null
        if (arrayDimensions > 0 && resolvedBase.sort == Type.VOID) {
            return null
        }
        var resolved = resolvedBase
        repeat(arrayDimensions) {
            resolved = Type.getType("[${resolved.descriptor}")
        }
        return resolved
    }

    private fun resolveBase(baseName: String): Type? {
        PRIMITIVE_TYPES[baseName]?.let { return it }
        if (baseName.startsWith("L") && !baseName.endsWith(";")) {
            return null
        }
        if (looksLikeJvmTypeDescriptor(baseName)) {
            return parseJvmTypeDescriptor(baseName)
        }
        if ('.' in baseName) {
            return Type.getObjectType(baseName.replace('.', '/'))
        }
        if ('/' in baseName) {
            return Type.getObjectType(baseName)
        }
        return null
    }

    private fun splitArraySuffix(typeName: String): Pair<String, Int> {
        var baseName = typeName
        var arrayDimensions = 0
        while (baseName.endsWith("[]")) {
            baseName = baseName.dropLast(2)
            arrayDimensions++
        }
        return baseName to arrayDimensions
    }

    private fun parseJvmTypeDescriptor(descriptor: String): Type? =
        try {
            when (val type = Type.getType(descriptor)) {
                Type.VOID_TYPE,
                Type.BOOLEAN_TYPE,
                Type.BYTE_TYPE,
                Type.CHAR_TYPE,
                Type.SHORT_TYPE,
                Type.INT_TYPE,
                Type.LONG_TYPE,
                Type.FLOAT_TYPE,
                Type.DOUBLE_TYPE,
                -> type
                else ->
                    when (type.sort) {
                        Type.ARRAY,
                        Type.OBJECT,
                        -> type
                        else -> null
                    }
            }
        } catch (_: IllegalArgumentException) {
            null
        }

    private fun looksLikeJvmTypeDescriptor(name: String): Boolean =
        when {
            name.startsWith("[") -> true
            name.length == 1 && name[0] in "BCDFIJSVL" -> true
            name.startsWith("L") && name.endsWith(";") -> true
            else -> false
        }
}

internal fun isMethodDescriptorLike(typeName: String): Boolean {
    if (typeName.startsWith("(")) {
        return true
    }
    val semicolonIndex = typeName.indexOf(';')
    if (semicolonIndex >= 0 && '(' in typeName.substring(semicolonIndex + 1)) {
        return true
    }
    return false
}

internal fun isValidClassLiteralResolvedType(type: Type): Boolean {
    if (type.sort == Type.METHOD) {
        return false
    }
    if (type.sort == Type.ARRAY && type.elementType.sort == Type.VOID) {
        return false
    }
    return when (type.sort) {
        Type.VOID,
        Type.BOOLEAN,
        Type.BYTE,
        Type.CHAR,
        Type.SHORT,
        Type.INT,
        Type.LONG,
        Type.FLOAT,
        Type.DOUBLE,
        Type.ARRAY,
        Type.OBJECT,
        -> true
        else -> false
    }
}

internal fun classLiteralTypeResolutionFailureMessage(typeName: String): String {
    if (isMethodDescriptorLike(typeName)) {
        return "method descriptor is not a type"
    }
    val baseName = typeName.substringBefore('[')
    if (looksLikeJvmTypeDescriptor(baseName)) {
        return "invalid type descriptor"
    }
    return "unresolved type name"
}

private fun looksLikeJvmTypeDescriptor(name: String): Boolean =
    when {
        name.startsWith("[") -> true
        name.length == 1 && name[0] in "BCDFIJSVL" -> true
        name.startsWith("L") -> true
        else -> false
    }
