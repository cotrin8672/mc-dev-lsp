package io.github.mcdev.jdtls.mixin

import java.util.Collections
import java.util.LinkedHashMap
import java.util.concurrent.ConcurrentHashMap

private const val MAX_CACHE_ENTRIES = 32

private const val INJECTION_POINT_FQN =
    "org.spongepowered.asm.mixin.injection.InjectionPoint"

private const val INJECTION_POINT_AT_CODE_FQN =
    "org.spongepowered.asm.mixin.injection.InjectionPoint\$AtCode"

private const val INJECTION_POINT_AT_CODE_DOT_FORM =
    "org.spongepowered.asm.mixin.injection.InjectionPoint.AtCode"

private const val AT_CODE_SIMPLE_NAME = "AtCode"

private const val SPONGE_INJECTION_PACKAGE_PREFIX =
    "org.spongepowered.asm.mixin.injection."

private fun <K, V> synchronizedAccessOrderLruMap(maxEntries: Int): MutableMap<K, V> {
    val backing = object : LinkedHashMap<K, V>(maxEntries, 0.75f, true) {
        override fun removeEldestEntry(eldest: MutableMap.MutableEntry<K, V>?): Boolean =
            size > maxEntries
    }
    return Collections.synchronizedMap(backing)
}

internal data class InjectionPointHierarchySubtype(
    val type: Any,
    val fqn: String,
)

internal fun interface InjectionPointHierarchyQuery {
    fun querySubtypes(javaProject: Any): List<InjectionPointHierarchySubtype>
}

internal object ReflectiveInjectionPointHierarchyQuery : InjectionPointHierarchyQuery {
    override fun querySubtypes(javaProject: Any): List<InjectionPointHierarchySubtype> {
        val injectionPointType = ReflectiveJavaProjectTypeLookup.findType(javaProject, INJECTION_POINT_FQN)
            ?: return emptyList()
        val iJavaProjectClass = Class.forName("org.eclipse.jdt.core.IJavaProject")
        val iProgressMonitorClass = Class.forName("org.eclipse.core.runtime.IProgressMonitor")
        val hierarchy = injectionPointType.javaClass.getMethod(
            "newTypeHierarchy",
            iJavaProjectClass,
            iProgressMonitorClass,
        ).invoke(injectionPointType, javaProject, null)
            ?: return emptyList()
        val iTypeClass = Class.forName("org.eclipse.jdt.core.IType")
        val subtypes = hierarchy.javaClass.getMethod("getAllSubtypes", iTypeClass)
            .invoke(hierarchy, injectionPointType) as? Array<*>
            ?: return emptyList()
        return subtypes.mapNotNull { subtype ->
            subtype ?: return@mapNotNull null
            val fqn = resolveTypeFqn(subtype) ?: return@mapNotNull null
            InjectionPointHierarchySubtype(type = subtype, fqn = fqn)
        }
    }
}

private fun resolveTypeFqn(type: Any): String? =
    runCatching {
        type.javaClass.getMethod("getFullyQualifiedName", Char::class.javaPrimitiveType)
            .invoke(type, '$') as String
    }.getOrNull()
        ?: runCatching {
            type.javaClass.getMethod("getFullyQualifiedName").invoke(type) as String
        }.getOrNull()

internal fun interface AtCodeTypeAnnotationReader {
    fun readDeclaredAtCode(type: Any, atCodeFqn: String): String?
}

internal object ReflectiveAtCodeTypeAnnotationReader : AtCodeTypeAnnotationReader {
    override fun readDeclaredAtCode(type: Any, atCodeFqn: String): String? {
        val annotations = type.javaClass.getMethod("getAnnotations")
            .invoke(type) as? Array<*>
            ?: return null
        val atCodeAnnotation = annotations.firstOrNull { annotation ->
            annotation != null &&
                annotationExists(annotation) &&
                matchesAtCodeElementName(
                    elementName = readElementName(annotation) ?: return@firstOrNull false,
                    atCodeFqn = atCodeFqn,
                )
        } ?: return null
        val pairs = atCodeAnnotation.javaClass.getMethod("getMemberValuePairs")
            .invoke(atCodeAnnotation) as? Array<*>
            ?: return null
        var namespace: String? = null
        var value: String? = null
        for (pair in pairs) {
            pair ?: continue
            val memberName = pair.javaClass.getMethod("getMemberName")
                .invoke(pair) as? String
                ?: continue
            val memberValue = pair.javaClass.getMethod("getValue")
                .invoke(pair) as? String
                ?: continue
            when (memberName) {
                "value" -> value = memberValue
                "namespace" -> namespace = memberValue
            }
        }
        val resolvedValue = value?.takeIf { it.isNotBlank() } ?: return null
        return formatAtCodeValue(namespace, resolvedValue)
    }
}

private fun annotationExists(annotation: Any): Boolean =
    runCatching {
        annotation.javaClass.getMethod("exists").invoke(annotation) as Boolean
    }.getOrDefault(true)

private fun readElementName(annotation: Any): String? =
    runCatching {
        annotation.javaClass.getMethod("getElementName").invoke(annotation) as String
    }.getOrNull()

private fun matchesAtCodeElementName(elementName: String, atCodeFqn: String): Boolean =
    elementName == atCodeFqn ||
        elementName == INJECTION_POINT_AT_CODE_FQN ||
        elementName == INJECTION_POINT_AT_CODE_DOT_FORM ||
        elementName == AT_CODE_SIMPLE_NAME

internal fun formatAtCodeValue(namespace: String?, value: String): String {
    val declaredNamespace = namespace?.takeIf { it.isNotBlank() }
    return if (declaredNamespace != null) {
        "$declaredNamespace:$value"
    } else {
        value
    }
}

internal class JdtInjectionPointAtCodeIndex(
    private val hierarchyQuery: InjectionPointHierarchyQuery = ReflectiveInjectionPointHierarchyQuery,
    private val atCodeReader: AtCodeTypeAnnotationReader = ReflectiveAtCodeTypeAnnotationReader,
) {
    private class JavaProjectIdentity(private val javaProject: Any) {
        override fun equals(other: Any?): Boolean =
            other is JavaProjectIdentity && javaProject === other.javaProject

        override fun hashCode(): Int = System.identityHashCode(javaProject)
    }

    private data class CacheKey(
        val javaProjectIdentity: JavaProjectIdentity,
        val projectSessionVersion: Long,
    )

    private val cache = synchronizedAccessOrderLruMap<CacheKey, List<String>>(MAX_CACHE_ENTRIES)
    private val inFlightLocks = ConcurrentHashMap<CacheKey, Any>()

    fun getValues(javaProject: Any?, projectSessionVersion: Long): List<String> {
        if (javaProject == null) {
            return emptyList()
        }
        val key = CacheKey(JavaProjectIdentity(javaProject), projectSessionVersion)
        synchronized(cache) {
            cache[key]?.let { return it }
        }
        val lock = inFlightLocks.computeIfAbsent(key) { Any() }
        return try {
            synchronized(lock) {
                synchronized(cache) {
                    cache[key]?.let { return it }
                }
                val built = runCatching { buildValues(javaProject) }
                if (built.isFailure) {
                    return emptyList()
                }
                val values = built.getOrThrow()
                synchronized(cache) {
                    cache[key] = values
                }
                values
            }
        } finally {
            inFlightLocks.remove(key, lock)
        }
    }

    private fun buildValues(javaProject: Any): List<String> {
        val subtypes = hierarchyQuery.querySubtypes(javaProject)
        if (subtypes.isEmpty()) {
            return emptyList()
        }
        val seen = linkedSetOf<String>()
        for (subtype in subtypes) {
            val declared = atCodeReader.readDeclaredAtCode(subtype.type, INJECTION_POINT_AT_CODE_FQN)
            when {
                declared != null -> seen.add(declared)
                !subtype.fqn.startsWith(SPONGE_INJECTION_PACKAGE_PREFIX) -> seen.add(subtype.fqn)
            }
        }
        return seen.sorted()
    }
}
