package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.InjectorModel
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.JavaModifier
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.JavaTypeKind
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixin.MixinMemberAnnotationKind
import io.github.mcdev.core.mixin.MixinMemberModel
import io.github.mcdev.core.mixin.MixinSemanticModelParser
import io.github.mcdev.core.mixin.MixinTargetRef
import io.github.mcdev.core.mixin.ParseConfidence
import io.github.mcdev.core.mixin.ParseSource
import io.github.mcdev.core.mixin.SemanticParseDebugInfo
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import java.lang.reflect.InvocationTargetException
import java.lang.reflect.UndeclaredThrowableException
import java.net.URI
import java.util.concurrent.CancellationException

data class JdtParseEnvironment(
    val classpathEntries: List<String> = emptyList(),
    val sourcepathEntries: List<String> = emptyList(),
    val unitName: String? = null,
)

class JdtMixinSemanticModelParser {
    private val astParserClass: Class<*>? = runCatching {
        Class.forName("org.eclipse.jdt.core.dom.ASTParser")
    }.getOrNull()
    private val astClass: Class<*>? = runCatching {
        Class.forName("org.eclipse.jdt.core.dom.AST")
    }.getOrNull()

    fun parse(
        source: String,
        documentUri: String,
        environment: JdtParseEnvironment = JdtParseEnvironment(),
    ): MixinClassModel {
        val parserClass = astParserClass
        val astApiClass = astClass
        if (parserClass == null || astApiClass == null) {
            return fallback(source, documentUri, "JDT ASTParser is not available in this runtime")
        }
        val jdtSource = resolveJdtSource(documentUri)
            ?: return fallback(source, documentUri, "JDT compilation unit is not available for $documentUri")
        return try {
            val ast = parseAst(parserClass, astApiClass, source, jdtSource, environment)
            val fallback = MixinSemanticModelParser.parse(
                source = source,
                sourceUri = documentUri,
                parseSource = ParseSource.JDT_AST,
                confidence = ParseConfidence.HIGH,
                debugInfo = SemanticParseDebugInfo(
                    parseSource = ParseSource.JDT_AST,
                    usedCompilationUnit = jdtSource.compilationUnit != null,
                    usedJavaProject = jdtSource.javaProject != null,
                ),
            )
            val astModel = AstModelExtractor(source, documentUri).extract(ast, fallback, jdtSource)
            astModel
        } catch (error: Throwable) {
            val cause = unwrapReflectionFailure(error)
            throwIfJdtAbort(cause)
            fallback(source, documentUri, "JDT AST parse failed: ${describeFailure(cause)}")
        }
    }

    private fun parseAst(
        parserClass: Class<*>,
        astApiClass: Class<*>,
        source: String,
        jdtSource: JdtAstSource,
        environment: JdtParseEnvironment,
    ): Any {
        val astLevel = astApiClass.fields.firstOrNull { it.name == "JLS21" }?.getInt(null)
            ?: astApiClass.fields.firstOrNull { it.name.startsWith("JLS") }?.getInt(null)
            ?: 21
        val parser = parserClass.getMethod("newParser", Int::class.javaPrimitiveType)
            .invoke(null, astLevel)
        val kind = parserClass.getField("K_COMPILATION_UNIT").getInt(null)
        parserClass.getMethod("setKind", Int::class.javaPrimitiveType).invoke(parser, kind)
        parserClass.getMethod("setSource", CharArray::class.java).invoke(parser, source.toCharArray())
        if (jdtSource.javaProject != null) {
            val javaProjectClass = Class.forName("org.eclipse.jdt.core.IJavaProject")
            parserClass.getMethod("setProject", javaProjectClass).invoke(parser, jdtSource.javaProject)
        } else {
            applyEnvironment(parserClass, parser, environment)
        }
        (environment.unitName ?: jdtSource.unitName)?.let {
            try {
                parserClass.getMethod("setUnitName", String::class.java).invoke(parser, it)
            } catch (error: Throwable) {
                throwIfJdtAbort(error)
            }
        }
        parserClass.getMethod("setResolveBindings", Boolean::class.javaPrimitiveType).invoke(parser, true)
        parserClass.getMethod("setBindingsRecovery", Boolean::class.javaPrimitiveType).invoke(parser, true)
        parserClass.getMethod("setStatementsRecovery", Boolean::class.javaPrimitiveType).invoke(parser, true)
        return parserClass.methods.firstOrNull { it.name == "createAST" && it.parameterCount == 1 }
            ?.invoke(parser, null)
            ?: error("ASTParser.createAST returned null")
    }

    private fun applyEnvironment(parserClass: Class<*>, parser: Any, environment: JdtParseEnvironment): Boolean {
        if (environment.classpathEntries.isEmpty() && environment.sourcepathEntries.isEmpty()) {
            return false
        }
        return try {
            parserClass.getMethod(
                "setEnvironment",
                Array<String>::class.java,
                Array<String>::class.java,
                Array<String>::class.java,
                Boolean::class.javaPrimitiveType,
            ).invoke(
                parser,
                environment.classpathEntries.toTypedArray(),
                environment.sourcepathEntries.toTypedArray(),
                null,
                true,
            )
            true
        } catch (error: Throwable) {
            throwIfJdtAbort(error)
            false
        }
    }

    private fun fallback(source: String, documentUri: String, reason: String): MixinClassModel =
        MixinSemanticModelParser.parse(
            source = source,
            sourceUri = documentUri,
            parseSource = ParseSource.HAND_WRITTEN_FALLBACK,
            confidence = ParseConfidence.LOW,
            warnings = listOf(reason),
            debugInfo = SemanticParseDebugInfo(
                parseSource = ParseSource.HAND_WRITTEN_FALLBACK,
                fallbackReason = reason,
            ),
        )

    private data class JdtAstSource(
        val compilationUnit: Any?,
        val javaProject: Any?,
        val unitName: String?,
    )

    private fun resolveJdtSource(documentUri: String): JdtAstSource? {
        return try {
            val javaCoreClass = Class.forName("org.eclipse.jdt.core.JavaCore")
            val resourcesPlugin = Class.forName("org.eclipse.core.resources.ResourcesPlugin")
            val workspace = resourcesPlugin.getMethod("getWorkspace").invoke(null) ?: return null
            val root = workspace.javaClass.getMethod("getRoot").invoke(workspace) ?: return null
            val uri = try {
                URI(documentUri)
            } catch (_: IllegalArgumentException) {
                return null
            }
            val files = root.javaClass.getMethod("findFilesForLocationURI", URI::class.java)
                .invoke(root, uri) as? Array<*>
            val file = files?.firstOrNull() ?: return null
            val compilationUnit = javaCoreClass.methods
                .firstOrNull { it.name == "createCompilationUnitFrom" && it.parameterCount == 1 }
                ?.invoke(null, file)
                ?: return null
            val javaProject = compilationUnit.javaClass.getMethod("getJavaProject").invoke(compilationUnit)
                ?: return null
            val unitName = compilationUnit.javaClass.getMethod("getElementName").invoke(compilationUnit) as? String
            JdtAstSource(compilationUnit, javaProject, unitName)
        } catch (error: Throwable) {
            throwIfJdtAbort(error)
            null
        }
    }

    internal fun resolveJavaProject(documentUri: String): Any? =
        resolveJdtSource(documentUri)?.javaProject

    companion object {
        private const val OPERATION_CANCELED_EXCEPTION = "org.eclipse.core.runtime.OperationCanceledException"

        internal fun unwrapReflectionFailure(throwable: Throwable): Throwable {
            var cause = throwable
            while (true) {
                val next = when (cause) {
                    is InvocationTargetException -> cause.targetException ?: cause.cause
                    is UndeclaredThrowableException -> cause.undeclaredThrowable ?: cause.cause
                    else -> null
                } ?: return cause
                if (next === cause) return cause
                cause = next
            }
        }

        internal fun describeFailure(throwable: Throwable): String {
            val cause = unwrapReflectionFailure(throwable)
            return cause.javaClass.name + (cause.message?.takeIf { it.isNotBlank() }?.let { ": $it" } ?: "")
        }

        internal fun throwIfJdtAbort(throwable: Throwable) {
            val cause = unwrapReflectionFailure(throwable)
            if (
                cause is Error ||
                cause is CancellationException ||
                cause is InterruptedException ||
                cause.javaClass.name == OPERATION_CANCELED_EXCEPTION
            ) {
                throw cause
            }
        }
    }

    private class AstModelExtractor(
        private val source: String,
        private val documentUri: String,
    ) {
        private val imports = JavaTypeDescriptorResolver.importsFor(source)
        private var bindingResolvedCount = 0
        private var bindingFailedCount = 0

        fun extract(root: Any, fallback: MixinClassModel, jdtSource: JdtAstSource): MixinClassModel {
            val nodes = flatten(root)
            val typeNode = nodes.firstOrNull { it.javaClass.simpleName in setOf("TypeDeclaration", "EnumDeclaration", "RecordDeclaration") }
            val packageName = packageName(root) ?: fallback.packageName
            val typeName = nodeName(typeNode).orEmpty()
            val qualifiedName = if (typeName.isBlank()) fallback.qualifiedName else {
                if (packageName.isBlank()) typeName else "$packageName.$typeName"
            }
            val targets = nodes.flatMap { node ->
                annotations(node)
                    .filter { annotationName(it) == "Mixin" }
                    .flatMap { mixinTargets(it) }
            }.ifEmpty { fallback.targets }
            val members = nodes.flatMap { node ->
                when (node.javaClass.simpleName) {
                    "MethodDeclaration" -> methodMember(node)
                    "FieldDeclaration" -> fieldMembers(node)
                    else -> emptyList()
                }
            }.ifEmpty { fallback.members }
            val methodNodes = nodes.filter { it.javaClass.simpleName == "MethodDeclaration" }
            val mixinExtras = JdtMixinExtrasResolvedContextCollector.collect(
                methodDeclarations = methodNodes,
                handlerRange = ::range,
                methodName = ::nodeName,
                source = source,
            )
            return fallback.copy(
                sourceUri = documentUri,
                packageName = packageName,
                qualifiedName = qualifiedName,
                typeKind = typeKind(typeNode),
                targets = targets,
                members = members,
                resolvedMixinExtrasContexts = mixinExtras.contexts,
                parseSource = ParseSource.JDT_AST,
                confidence = if (bindingFailedCount > 0 || members.any { it.confidence != ParseConfidence.HIGH }) ParseConfidence.MEDIUM else ParseConfidence.HIGH,
                warnings = (members.flatMap { it.warnings } + mixinExtras.warnings).distinct(),
                debugInfo = SemanticParseDebugInfo(
                    parseSource = ParseSource.JDT_AST,
                    usedCompilationUnit = jdtSource.compilationUnit != null,
                    usedJavaProject = jdtSource.javaProject != null,
                    bindingResolvedCount = bindingResolvedCount,
                    bindingFailedCount = bindingFailedCount,
                ),
            )
        }

        private fun methodMember(node: Any): List<MixinMemberModel> {
            val annotation = annotations(node).firstNotNullOfOrNull { annotation ->
                when (annotationName(annotation)) {
                    "Accessor" -> MixinMemberAnnotationKind.ACCESSOR to annotation
                    "Invoker" -> MixinMemberAnnotationKind.INVOKER to annotation
                    "Shadow" -> MixinMemberAnnotationKind.SHADOW to annotation
                    "Overwrite" -> MixinMemberAnnotationKind.OVERWRITE to annotation
                    else -> null
                }
            } ?: return emptyList()
            val methodName = nodeName(node) ?: return emptyList()
            val parameterDescriptors = parameterTypes(node).mapNotNull(::descriptorForType)
            val returnDescriptor = returnType(node)?.let(::descriptorForType)
            val descriptor = returnDescriptor?.let { "(${parameterDescriptors.joinToString("")})$it" }
            val warning = if (descriptor == null) listOf("JDT AST could not resolve descriptor for method $methodName") else emptyList()
            return listOf(
                MixinMemberModel(
                    annotationKind = annotation.first,
                    javaName = methodName,
                    explicitTargetName = stringValue(annotation.second),
                    returnDescriptor = returnDescriptor,
                    parameterDescriptors = parameterDescriptors,
                    methodDescriptor = descriptor,
                    modifiers = modifiers(node),
                    range = range(node),
                    annotationRange = range(annotation.second),
                    nameRange = nameRange(node),
                    parseSource = ParseSource.JDT_AST,
                    confidence = if (descriptor == null) ParseConfidence.LOW else ParseConfidence.HIGH,
                    warnings = warning,
                ),
            )
        }

        private fun fieldMembers(node: Any): List<MixinMemberModel> {
            val annotation = annotations(node).firstNotNullOfOrNull { annotation ->
                when (annotationName(annotation)) {
                    "Shadow" -> MixinMemberAnnotationKind.SHADOW to annotation
                    else -> null
                }
            } ?: return emptyList()
            val typeDescriptor = fieldType(node)?.let(::descriptorForType)
            val warning = if (typeDescriptor == null) listOf("JDT AST could not resolve descriptor for field declaration") else emptyList()
            return fragments(node).mapNotNull { fragment ->
                val name = nodeName(fragment) ?: return@mapNotNull null
                MixinMemberModel(
                    annotationKind = annotation.first,
                    javaName = name,
                    explicitTargetName = name,
                    returnDescriptor = typeDescriptor,
                    parameterDescriptors = emptyList(),
                    methodDescriptor = null,
                    modifiers = modifiers(node),
                    range = range(node),
                    annotationRange = range(annotation.second),
                    nameRange = nameRange(fragment),
                    parseSource = ParseSource.JDT_AST,
                    confidence = if (typeDescriptor == null) ParseConfidence.LOW else ParseConfidence.HIGH,
                    warnings = warning,
                )
            }
        }

        private fun mixinTargets(annotation: Any): List<MixinTargetRef> =
            annotationValues(annotation).mapNotNull { expression ->
                val typeLiteral = expression.javaClass.simpleName == "TypeLiteral"
                val raw = if (typeLiteral) {
                    expression.toString().removeSuffix(".class").trim('"')
                } else {
                    JdtAnnotationConstantExtractor.constantString(expression)
                        ?: expression.toString().removeSuffix(".class").trim('"')
                }
                val internal = if (typeLiteral) {
                    typeBindingInternalName(expression)
                        ?: imports.explicit[raw]?.let(AnnotationContextExtractor::fqnToInternal)
                        ?: raw
                } else {
                    raw.replace('.', '/')
                }
                MixinTargetRef(internalName = internal, range = range(expression))
            }

        private fun descriptorForType(typeNode: Any): String? {
            typeBindingDescriptor(typeNode)?.let { return it }
            val raw = typeNode.toString().removeSuffix("...")
            return JavaTypeDescriptorResolver.descriptorOrNull(raw, imports)
        }

        private fun typeBindingDescriptor(node: Any): String? {
            val binding = JdtAnnotationConstantExtractor.typeBinding(node) ?: run {
                bindingFailedCount++
                return null
            }
            bindingResolvedCount++
            return JdtBindingDescriptorConverter.descriptorFromBinding(binding)
        }

        private fun typeBindingInternalName(node: Any): String? {
            val binding = JdtAnnotationConstantExtractor.typeBinding(node) ?: run {
                bindingFailedCount++
                return null
            }
            bindingResolvedCount++
            return bindingBinaryName(binding)?.replace('.', '/')
        }

        private fun bindingBinaryName(binding: Any): String? =
            string(binding, "getBinaryName")
                ?: string(binding, "getQualifiedName")

        private fun flatten(root: Any): List<Any> {
            val results = mutableListOf<Any>()
            fun visit(node: Any?) {
                if (node == null) return
                results += node
                for (property in structuralProperties(node)) {
                    val value = getStructuralProperty(node, property) ?: continue
                    when (value) {
                        is Iterable<*> -> value.forEach(::visit)
                        else -> if (value.javaClass.name.startsWith("org.eclipse.jdt.core.dom.")) visit(value)
                    }
                }
            }
            visit(root)
            return results
        }

        private fun annotations(node: Any?): List<Any> =
            listProperty(node, "modifiers").filter { it.javaClass.simpleName.endsWith("Annotation") }

        private fun fragments(node: Any): List<Any> =
            listProperty(node, "fragments")

        private fun parameterTypes(node: Any): List<Any> =
            listProperty(node, "parameters").mapNotNull { call(it, "getType") }

        private fun returnType(node: Any): Any? =
            call(node, "getReturnType2") ?: call(node, "getReturnType")

        private fun fieldType(node: Any): Any? =
            call(node, "getType")

        private fun annotationValues(annotation: Any): List<Any> {
            return when (annotation.javaClass.simpleName) {
                "SingleMemberAnnotation" -> listOfNotNull(call(annotation, "getValue")).flatMap(::arrayExpressions)
                "NormalAnnotation" -> listProperty(annotation, "values").flatMap { pair ->
                    val name = nodeName(pair)
                    if (name == "value" || name == "targets" || name == "target") {
                        arrayExpressions(call(pair, "getValue"))
                    } else {
                        emptyList()
                    }
                }
                else -> emptyList()
            }
        }

        private fun stringValue(annotation: Any): String? =
            annotationValues(annotation).firstOrNull()?.toString()?.trim('"')

        private fun arrayExpressions(expression: Any?): List<Any> {
            if (expression == null) return emptyList()
            return if (expression.javaClass.simpleName == "ArrayInitializer") {
                listProperty(expression, "expressions")
            } else {
                listOf(expression)
            }
        }

        private fun annotationName(annotation: Any): String =
            call(annotation, "getTypeName")?.toString()?.substringAfterLast('.') ?: ""

        private fun packageName(root: Any): String? =
            call(root, "getPackage")?.let { call(it, "getName")?.toString() }

        private fun typeKind(node: Any?): JavaTypeKind =
            when (node?.javaClass?.simpleName) {
                "EnumDeclaration" -> JavaTypeKind.ENUM
                "RecordDeclaration" -> JavaTypeKind.RECORD
                "TypeDeclaration" -> if (bool(node, "isInterface")) JavaTypeKind.INTERFACE else JavaTypeKind.CLASS
                else -> JavaTypeKind.UNKNOWN
            }

        private fun modifiers(node: Any): Set<JavaModifier> =
            listProperty(node, "modifiers").mapNotNull { modifier ->
                if (modifier.javaClass.simpleName != "Modifier") return@mapNotNull null
                when (modifier.toString()) {
                    "public" -> JavaModifier.PUBLIC
                    "protected" -> JavaModifier.PROTECTED
                    "private" -> JavaModifier.PRIVATE
                    "abstract" -> JavaModifier.ABSTRACT
                    "static" -> JavaModifier.STATIC
                    "final" -> JavaModifier.FINAL
                    "synchronized" -> JavaModifier.SYNCHRONIZED
                    "native" -> JavaModifier.NATIVE
                    "strictfp" -> JavaModifier.STRICTFP
                    "transient" -> JavaModifier.TRANSIENT
                    "volatile" -> JavaModifier.VOLATILE
                    else -> null
                }
            }.toSet()

        private fun nodeName(node: Any?): String? {
            if (node == null) return null
            return call(node, "getName")?.toString()
        }

        private fun range(node: Any): McTextRange {
            val start = int(node, "getStartPosition")?.coerceAtLeast(0) ?: 0
            val length = int(node, "getLength")?.coerceAtLeast(0) ?: 0
            return offsetRange(start, start + length)
        }

        private fun nameRange(node: Any): McTextRange =
            call(node, "getName")?.let(::range) ?: range(node)

        private fun offsetRange(start: Int, end: Int): McTextRange =
            McTextRange(offsetToPosition(start), offsetToPosition(end.coerceAtMost(source.length)))

        private fun offsetToPosition(offset: Int): McTextPosition {
            var line = 0
            var character = 0
            var i = 0
            val safeOffset = offset.coerceIn(0, source.length)
            while (i < safeOffset) {
                if (source[i] == '\n') {
                    line++
                    character = 0
                } else {
                    character++
                }
                i++
            }
            return McTextPosition(line, character)
        }

        private fun structuralProperties(node: Any): List<Any> =
            runCatching {
                @Suppress("UNCHECKED_CAST")
                node.javaClass.getMethod("structuralPropertiesForType").invoke(node) as? List<Any>
            }.getOrNull().orEmpty()

        private fun getStructuralProperty(node: Any, property: Any): Any? =
            runCatching {
                node.javaClass.methods
                    .firstOrNull { it.name == "getStructuralProperty" && it.parameterCount == 1 }
                    ?.invoke(node, property)
            }
                .getOrNull()

        private fun listProperty(node: Any?, name: String): List<Any> {
            if (node == null) return emptyList()
            return runCatching {
                val value = node.javaClass.getMethod(name).apply { trySetAccessible() }.invoke(node)
                @Suppress("UNCHECKED_CAST")
                value as? List<Any>
            }.onFailure { throwIfJdtAbort(it) }.getOrNull().orEmpty()
        }

        private fun call(node: Any, name: String): Any? =
            runCatching {
                node.javaClass.methods
                    .firstOrNull { it.name == name && it.parameterCount == 0 }
                    ?.apply { trySetAccessible() }
                    ?.invoke(node)
            }
                .onFailure { throwIfJdtAbort(it) }
                .getOrNull()

        private fun string(node: Any, name: String): String? =
            call(node, name) as? String

        private fun int(node: Any, name: String): Int? =
            call(node, name) as? Int

        private fun bool(node: Any, name: String): Boolean =
            call(node, name) as? Boolean ?: false
    }
}

internal data class CollectedMixinExtrasContexts(
    val contexts: List<ResolvedMixinExtrasContext>,
    val warnings: List<String>,
)

internal object JdtMixinExtrasResolvedContextCollector {
    fun collect(
        methodDeclarations: Iterable<Any>,
        handlerRange: (Any) -> McTextRange,
        methodName: (Any) -> String? = { node ->
            runCatching {
                node.javaClass.methods
                    .firstOrNull { it.name == "getName" && it.parameterCount == 0 }
                    ?.invoke(node)
                    ?.toString()
            }.getOrNull()
        },
        source: String? = null,
        extract: (Any, McTextRange) -> JdtMixinExtrasResolvedContextResult = { method, range ->
            JdtMixinExtrasResolvedContextExtractor.extract(method, range, source)
        },
    ): CollectedMixinExtrasContexts {
        val contexts = mutableListOf<ResolvedMixinExtrasContext>()
        val warnings = mutableListOf<String>()
        for (method in methodDeclarations) {
            val range = handlerRange(method)
            when (val result = extract(method, range)) {
                JdtMixinExtrasResolvedContextResult.NotMixinExtrasHandler -> Unit
                is JdtMixinExtrasResolvedContextResult.Unavailable -> {
                    val name = methodName(method)
                    warnings += if (name != null) {
                        "MixinExtras context unavailable for method $name: ${result.reason}"
                    } else {
                        "MixinExtras context unavailable: ${result.reason}"
                    }
                }
                is JdtMixinExtrasResolvedContextResult.Resolved -> contexts += result.context
            }
        }
        return CollectedMixinExtrasContexts(contexts, warnings)
    }
}
