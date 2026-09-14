package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.jdtls.project.ClassMemberIndexAdapter
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.Proxy
import java.util.concurrent.CancellationException
import java.util.concurrent.atomic.AtomicBoolean

internal class JdtProjectSourceQuery(
    private val javaProject: Any,
    private val isJdtSearchEngineAvailable: () -> Boolean = Companion::defaultJdtSearchEngineAvailable,
) : ProjectSourceQuery {
    private val lastSourceQueryFailure = ThreadLocal.withInitial { false }

    override fun coversProjectDependencies(): Boolean {
        val failed = lastSourceQueryFailure.get()
        lastSourceQueryFailure.remove()
        return isJdtSearchEngineAvailable() && !failed
    }

    override fun findClasses(prefix: String, limit: Int): Sequence<ClassIndexEntry> {
        lastSourceQueryFailure.set(false)
        return findClassesByPrefix(prefix, limit).asSequence()
    }

    private fun findClassesByPrefix(prefix: String, limit: Int): List<ClassIndexEntry> {
        if (limit <= 0) {
            return emptyList()
        }
        return try {
            searchAllTypeNamesByPrefix(prefix, limit)
        } catch (throwable: Throwable) {
            val cause = unwrapInvocationTarget(throwable)
            if (
                cause is Error ||
                    cause is CancellationException ||
                    cause is InterruptedException ||
                    cause.javaClass.name == OPERATION_CANCELED_EXCEPTION
            ) {
                throw cause
            }
            lastSourceQueryFailure.set(true)
            emptyList()
        }
    }

    private fun searchAllTypeNamesByPrefix(prefix: String, limit: Int): List<ClassIndexEntry> {
        val searchEngineClass = Class.forName("org.eclipse.jdt.core.search.SearchEngine")
        val scopeClass = Class.forName("org.eclipse.jdt.core.search.IJavaSearchScope")
        val requestorClass = Class.forName("org.eclipse.jdt.core.search.ITypeNameRequestor")
        val monitorClass = Class.forName("org.eclipse.core.runtime.IProgressMonitor")
        val javaElementClass = Class.forName("org.eclipse.jdt.core.IJavaElement")

        val projectElements = java.lang.reflect.Array.newInstance(javaElementClass, 1)
        java.lang.reflect.Array.set(projectElements, 0, javaProject)

        val scopeFlags = prefixSearchScopeFlags(scopeClass) ?: run {
            lastSourceQueryFailure.set(true)
            return emptyList()
        }

        val scope = searchEngineClass.getMethod(
            "createJavaSearchScope",
            Class.forName("[Lorg.eclipse.jdt.core.IJavaElement;"),
            Int::class.javaPrimitiveType,
        ).invoke(null, projectElements, scopeFlags)
            ?: run {
                lastSourceQueryFailure.set(true)
                return emptyList()
            }

        val (packagePrefix, typePrefix) = prefixToSearchChars(prefix)
        val seenInternalNames = LinkedHashSet<String>()
        val results = mutableListOf<ClassIndexEntry>()
        val cancelled = AtomicBoolean(false)

        val requestor = Proxy.newProxyInstance(
            searchEngineClass.classLoader,
            arrayOf(requestorClass),
        ) { _, method, args ->
            when (method.name) {
                "acceptClass", "acceptInterface" -> {
                    if (cancelled.get()) {
                        return@newProxyInstance null
                    }
                    @Suppress("UNCHECKED_CAST")
                    val enclosingTypes = args[2] as? Array<CharArray>
                    val entry = requestorTypeToClassIndexEntry(
                        packageName = args[0] as CharArray,
                        typeName = args[1] as CharArray,
                        enclosingTypeNames = enclosingTypes,
                    )
                    if (seenInternalNames.add(entry.internalName)) {
                        results.add(entry)
                        if (results.size >= limit) {
                            cancelled.set(true)
                        }
                    }
                    null
                }
                else -> null
            }
        }

        val monitor = Proxy.newProxyInstance(
            searchEngineClass.classLoader,
            arrayOf(monitorClass),
        ) { _, method, _ ->
            when (method.name) {
                "isCanceled" -> cancelled.get()
                else -> defaultProxyValue(method.returnType)
            }
        }

        val searchEngine = searchEngineClass.getConstructor().newInstance()

        try {
            searchEngineClass.getMethod(
                "searchAllTypeNames",
                CharArray::class.java,
                CharArray::class.java,
                Int::class.javaPrimitiveType,
                Int::class.javaPrimitiveType,
                scopeClass,
                requestorClass,
                Int::class.javaPrimitiveType,
                monitorClass,
            ).invoke(
                searchEngine,
                packagePrefix,
                typePrefix,
                R_PREFIX_MATCH,
                TYPE,
                scope,
                requestor,
                FORCE_IMMEDIATE_SEARCH,
                monitor,
            )
        } catch (throwable: Throwable) {
            val cause = unwrapInvocationTarget(throwable)
            if (cancelled.get() && cause.javaClass.name == OPERATION_CANCELED_EXCEPTION) {
                return sortClassIndexEntries(results)
            }
            throw cause
        }

        return sortClassIndexEntries(results)
    }

    private fun defaultProxyValue(returnType: Class<*>): Any? =
        when (returnType) {
            Boolean::class.javaPrimitiveType -> false
            Int::class.javaPrimitiveType -> 0
            else -> null
        }

    override fun findClass(internalName: String): ClassIndexEntry? {
        lastSourceQueryFailure.set(false)
        return findSourceType(internalNameToFqn(internalName))?.let(::toClassIndexEntry)
    }

    override fun findClassByFqn(fqn: String): ClassIndexEntry? {
        lastSourceQueryFailure.set(false)
        return findSourceType(fqn)?.let(::toClassIndexEntry)
    }

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> {
        lastSourceQueryFailure.set(false)
        val type = findSourceType(internalNameToFqn(ownerInternalName)) ?: return emptyList()
        val methods = invokeGetMethods(type) ?: run {
            lastSourceQueryFailure.set(true)
            return emptyList()
        }
        val resolver = typeResolver(type)
        return methods.mapNotNull { method ->
            if (method == null) {
                return@mapNotNull null
            }
            if (invokeIsConstructor(method) != false) {
                return@mapNotNull null
            }
            val name = invokeElementName(method) ?: return@mapNotNull null
            val descriptor = methodDescriptor(method, resolver) ?: return@mapNotNull null
            val flags = invokeFlags(method) ?: return@mapNotNull null
            MethodIndexEntry(
                name = name,
                descriptor = descriptor,
                isStatic = flags and ACC_STATIC != 0,
                readableSignature = ClassMemberIndexAdapter.readableMethodSignature(name, descriptor),
            )
        }
    }

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> {
        lastSourceQueryFailure.set(false)
        val type = findSourceType(internalNameToFqn(ownerInternalName)) ?: return emptyList()
        val fields = invokeGetFields(type) ?: run {
            lastSourceQueryFailure.set(true)
            return emptyList()
        }
        val resolver = typeResolver(type)
        return fields.mapNotNull { field ->
            if (field == null) {
                return@mapNotNull null
            }
            val name = invokeElementName(field) ?: return@mapNotNull null
            val typeSignature = invokeTypeSignature(field) ?: return@mapNotNull null
            val descriptor = JdtTypeSignatureConverter.toJvmDescriptorOrNull(typeSignature, resolver)
                ?: return@mapNotNull null
            val flags = invokeFlags(field) ?: return@mapNotNull null
            FieldIndexEntry(
                name = name,
                descriptor = descriptor,
                isStatic = flags and ACC_STATIC != 0,
                readableType = ClassMemberIndexAdapter.readableFieldType(descriptor),
            )
        }
    }

    private fun findSourceType(fqn: String): Any? =
        try {
            javaProject.javaClass.getMethod("findType", String::class.java).invoke(javaProject, fqn)
                .also { if (it == null) lastSourceQueryFailure.set(true) }
        } catch (throwable: Throwable) {
            val cause = unwrapInvocationTarget(throwable)
            if (
                cause is Error ||
                    cause is CancellationException ||
                    cause is InterruptedException ||
                    cause.javaClass.name == OPERATION_CANCELED_EXCEPTION
            ) {
                throw cause
            }
            lastSourceQueryFailure.set(true)
            null
        }

    private fun unwrapInvocationTarget(throwable: Throwable): Throwable {
        var cause = throwable
        while (cause is InvocationTargetException) {
            val target = cause.targetException ?: break
            cause = target
        }
        return cause
    }

    private fun prefixSearchScopeFlags(scopeClass: Class<*>): Int? {
        val sources = reflectIntConstant(scopeClass, IJavaSearchScopeField.SOURCES) ?: return null
        val applicationLibraries =
            reflectIntConstant(scopeClass, IJavaSearchScopeField.APPLICATION_LIBRARIES) ?: return null
        return sources or applicationLibraries
    }

    private fun reflectIntConstant(clazz: Class<*>, fieldName: String): Int? =
        try {
            clazz.getField(fieldName).getInt(null)
        } catch (throwable: Throwable) {
            val cause = unwrapInvocationTarget(throwable)
            if (
                cause is Error ||
                    cause is CancellationException ||
                    cause is InterruptedException ||
                    cause.javaClass.name == OPERATION_CANCELED_EXCEPTION
            ) {
                throw cause
            }
            null
        }

    private fun toClassIndexEntry(type: Any): ClassIndexEntry? {
        val packageName = invokePackageName(type)
        val packageRelative = packageRelativeTypeName(type, packageName) ?: return null
        val simpleName = packageRelative.ifEmpty { return null }
        return ClassIndexEntry(
            simpleName = simpleName,
            packageName = packageName,
            internalName = internalNameFromPackageAndType(packageName, simpleName),
        )
    }

    private fun packageRelativeTypeName(type: Any, packageName: String): String? {
        invokeFullyQualifiedName(type, enclosingSeparator = '$')?.let { dollarFqn ->
            val relative = if (packageName.isEmpty()) {
                dollarFqn
            } else {
                dollarFqn.removePrefix("$packageName.")
            }
            if (relative.isNotEmpty()) {
                return relative
            }
        }
        val dotFqn = invokeFullyQualifiedName(type, enclosingSeparator = '.') ?: return null
        val relative = if (packageName.isEmpty()) {
            dotFqn
        } else {
            dotFqn.removePrefix("$packageName.")
        }
        if (relative.isEmpty()) {
            return null
        }
        return relative.replace('.', '$')
    }

    private fun methodDescriptor(method: Any, resolver: UnresolvedErasureNameResolver): String? {
        val parameterSignatures = invokeParameterTypes(method) ?: return null
        val returnSignature = invokeReturnType(method) ?: return null
        val parameterDescriptors = parameterSignatures.map { signature ->
            JdtTypeSignatureConverter.toJvmDescriptorOrNull(signature, resolver) ?: return null
        }
        val returnDescriptor = JdtTypeSignatureConverter.toJvmDescriptorOrNull(returnSignature, resolver) ?: return null
        return "(${parameterDescriptors.joinToString("")})$returnDescriptor"
    }

    private fun typeResolver(enclosingType: Any): UnresolvedErasureNameResolver =
        { erasureName -> resolveTypeToInternalName(enclosingType, erasureName) }

    private fun resolveTypeToInternalName(enclosingType: Any, erasureName: String): String? {
        val resolved = runCatching {
            enclosingType.javaClass.getMethod("resolveType", String::class.java)
                .invoke(enclosingType, erasureName) as? Array<*>
        }.getOrNull() ?: return null
        val pair = uniquePackageTypePair(resolved) ?: return null
        return internalNameFromPackageAndType(pair.first, pair.second)
    }

    private fun uniquePackageTypePair(resolved: Array<*>): Pair<String, String>? {
        val pairs = resolved.mapNotNull { entry ->
            val pairArray = entry as? Array<*> ?: return@mapNotNull null
            if (pairArray.size != 2) {
                return@mapNotNull null
            }
            val packageName = pairArray[0] as? String ?: return@mapNotNull null
            val typeName = pairArray[1] as? String ?: return@mapNotNull null
            packageName to typeName
        }.distinct()
        return pairs.singleOrNull()
    }

    private fun internalNameFromPackageAndType(packageName: String, typeName: String): String {
        val typeInternal = typeName.replace('.', '$')
        return if (packageName.isEmpty()) {
            typeInternal
        } else {
            "${packageName.replace('.', '/')}/$typeInternal"
        }
    }

    private fun internalNameToFqn(internalName: String): String {
        val slashIndex = internalName.lastIndexOf('/')
        if (slashIndex < 0) {
            return internalName.replace('$', '.')
        }
        val packagePath = internalName.substring(0, slashIndex)
        val classPart = internalName.substring(slashIndex + 1)
        val packageName = packagePath.replace('/', '.')
        val className = classPart.replace('$', '.')
        return if (packageName.isEmpty()) className else "$packageName.$className"
    }

    private fun invokeIsConstructor(method: Any): Boolean? =
        runCatching { method.javaClass.getMethod("isConstructor").invoke(method) as Boolean }.getOrNull()

    private fun invokeFlags(method: Any): Int? =
        runCatching { method.javaClass.getMethod("getFlags").invoke(method) as Int }.getOrNull()

    private fun invokeElementName(element: Any): String? =
        runCatching { element.javaClass.getMethod("getElementName").invoke(element) as String }.getOrNull()

    private fun invokePackageName(type: Any): String {
        val packageFragment = runCatching {
            type.javaClass.getMethod("getPackageFragment").invoke(type)
        }.getOrNull() ?: return ""
        return runCatching {
            packageFragment.javaClass.getMethod("getElementName").invoke(packageFragment) as String
        }.getOrDefault("")
    }

    private fun invokeFullyQualifiedName(type: Any, enclosingSeparator: Char): String? =
        runCatching {
            type.javaClass.getMethod(
                "getFullyQualifiedName",
                Char::class.javaPrimitiveType,
                Boolean::class.javaPrimitiveType,
            ).invoke(type, enclosingSeparator, true) as String
        }.getOrNull()
            ?: runCatching {
                type.javaClass.getMethod(
                    "getFullyQualifiedName",
                    Char::class.javaPrimitiveType,
                ).invoke(type, enclosingSeparator) as String
            }.getOrNull()
            ?: runCatching {
                type.javaClass.getMethod("getFullyQualifiedName").invoke(type) as String
            }.getOrNull()

    private fun invokeGetMethods(type: Any): Array<*>? =
        runCatching { type.javaClass.getMethod("getMethods").invoke(type) as Array<*> }.getOrNull()

    private fun invokeGetFields(type: Any): Array<*>? =
        runCatching { type.javaClass.getMethod("getFields").invoke(type) as Array<*> }.getOrNull()

    private fun invokeTypeSignature(field: Any): String? =
        runCatching { field.javaClass.getMethod("getTypeSignature").invoke(field) as String }.getOrNull()

    private fun invokeParameterTypes(method: Any): Array<String>? {
        val raw = runCatching {
            method.javaClass.getMethod("getParameterTypes").invoke(method) as? Array<*>
        }.getOrNull() ?: return null
        return raw.map { element ->
            element as? String ?: return null
        }.toTypedArray()
    }

    private fun invokeReturnType(method: Any): String? =
        runCatching { method.javaClass.getMethod("getReturnType").invoke(method) as String }.getOrNull()

    internal companion object {
        internal fun defaultJdtSearchEngineAvailable(): Boolean =
            runCatching { Class.forName("org.eclipse.jdt.core.search.SearchEngine") }.isSuccess

        private const val ACC_STATIC = 0x0008
        private const val R_PREFIX_MATCH = 1
        private const val TYPE = 0
        private const val FORCE_IMMEDIATE_SEARCH = 1
        private const val OPERATION_CANCELED_EXCEPTION =
            "org.eclipse.core.runtime.OperationCanceledException"

        private object IJavaSearchScopeField {
            const val SOURCES = "SOURCES"
            const val APPLICATION_LIBRARIES = "APPLICATION_LIBRARIES"
        }

        internal fun prefixToSearchChars(prefix: String): Pair<CharArray?, CharArray> {
            val normalized = prefix.replace('/', '.')
            val lastDot = normalized.lastIndexOf('.')
            return if (lastDot >= 0) {
                normalized.substring(0, lastDot).toCharArray() to normalized.substring(lastDot + 1).toCharArray()
            } else {
                null to normalized.toCharArray()
            }
        }

        internal fun requestorTypeToClassIndexEntry(
            packageName: CharArray,
            typeName: CharArray,
            enclosingTypeNames: Array<CharArray>?,
        ): ClassIndexEntry {
            val packageText = String(packageName)
            val simpleName = packageRelativeTypeName(String(typeName), enclosingTypeNames)
            return ClassIndexEntry(
                simpleName = simpleName,
                packageName = packageText,
                internalName = internalNameFromPackageAndType(packageText, simpleName),
            )
        }

        internal fun sortClassIndexEntries(entries: List<ClassIndexEntry>): List<ClassIndexEntry> =
            entries.sortedBy { it.internalName }

        private fun packageRelativeTypeName(
            simpleTypeName: String,
            enclosingTypeNames: Array<CharArray>?,
        ): String {
            val enclosingNames = enclosingTypeNames?.map(::String).orEmpty()
            return if (enclosingNames.isEmpty()) {
                simpleTypeName
            } else {
                enclosingNames.joinToString("$") + "$" + simpleTypeName
            }
        }

        private fun internalNameFromPackageAndType(packageName: String, typeName: String): String {
            val typeInternal = typeName.replace('.', '$')
            return if (packageName.isEmpty()) {
                typeInternal
            } else {
                "${packageName.replace('.', '/')}/$typeInternal"
            }
        }
    }
}
