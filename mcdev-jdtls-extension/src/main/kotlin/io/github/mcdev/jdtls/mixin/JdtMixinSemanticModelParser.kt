package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.InjectorModel
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

class JdtMixinSemanticModelParser {
    private val astParserClass: Class<*>? = runCatching {
        Class.forName("org.eclipse.jdt.core.dom.ASTParser")
    }.getOrNull()
    private val astClass: Class<*>? = runCatching {
        Class.forName("org.eclipse.jdt.core.dom.AST")
    }.getOrNull()

    fun parse(source: String, documentUri: String): MixinClassModel {
        val parserClass = astParserClass
        val astApiClass = astClass
        if (parserClass == null || astApiClass == null) {
            return fallback(source, documentUri, "JDT ASTParser is not available in this runtime")
        }
        return runCatching {
            val ast = parseAst(parserClass, astApiClass, source)
            val fallback = MixinSemanticModelParser.parse(
                source = source,
                sourceUri = documentUri,
                parseSource = ParseSource.JDT_AST,
                confidence = ParseConfidence.HIGH,
            )
            val astModel = AstModelExtractor(source, documentUri).extract(ast, fallback)
            astModel
        }.getOrElse { error ->
            fallback(source, documentUri, "JDT AST parse failed: ${error.message ?: error.javaClass.name}")
        }
    }

    private fun parseAst(parserClass: Class<*>, astApiClass: Class<*>, source: String): Any {
        val astLevel = astApiClass.fields.firstOrNull { it.name == "JLS21" }?.getInt(null)
            ?: astApiClass.fields.firstOrNull { it.name.startsWith("JLS") }?.getInt(null)
            ?: 21
        val parser = parserClass.getMethod("newParser", Int::class.javaPrimitiveType)
            .invoke(null, astLevel)
        val kind = parserClass.getField("K_COMPILATION_UNIT").getInt(null)
        parserClass.getMethod("setKind", Int::class.javaPrimitiveType).invoke(parser, kind)
        parserClass.getMethod("setSource", CharArray::class.java).invoke(parser, source.toCharArray())
        parserClass.getMethod("setResolveBindings", Boolean::class.javaPrimitiveType).invoke(parser, true)
        parserClass.getMethod("setBindingsRecovery", Boolean::class.javaPrimitiveType).invoke(parser, true)
        parserClass.getMethod("setStatementsRecovery", Boolean::class.javaPrimitiveType).invoke(parser, true)
        return parserClass.methods.firstOrNull { it.name == "createAST" && it.parameterCount == 1 }
            ?.invoke(parser, null)
            ?: error("ASTParser.createAST returned null")
    }

    private fun fallback(source: String, documentUri: String, reason: String): MixinClassModel =
        MixinSemanticModelParser.parse(
            source = source,
            sourceUri = documentUri,
            parseSource = ParseSource.HAND_WRITTEN_FALLBACK,
            confidence = ParseConfidence.LOW,
            warnings = listOf(reason),
        )

    private class AstModelExtractor(
        private val source: String,
        private val documentUri: String,
    ) {
        private val imports = JavaTypeDescriptorResolver.importsFor(source)

        fun extract(root: Any, fallback: MixinClassModel): MixinClassModel {
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
            return fallback.copy(
                sourceUri = documentUri,
                packageName = packageName,
                qualifiedName = qualifiedName,
                typeKind = typeKind(typeNode),
                targets = targets,
                members = members,
                parseSource = ParseSource.JDT_AST,
                confidence = if (members.any { it.confidence != ParseConfidence.HIGH }) ParseConfidence.MEDIUM else ParseConfidence.HIGH,
                warnings = members.flatMap { it.warnings }.distinct(),
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
                val raw = expression.toString().removeSuffix(".class").trim('"')
                val internal = typeBindingInternalName(expression) ?: raw.replace('.', '/')
                MixinTargetRef(internalName = internal, range = range(expression))
            }

        private fun descriptorForType(typeNode: Any): String? {
            typeBindingDescriptor(typeNode)?.let { return it }
            val raw = typeNode.toString().removeSuffix("...")
            return JavaTypeDescriptorResolver.descriptorOrNull(raw, imports)
        }

        private fun typeBindingDescriptor(node: Any): String? {
            val binding = runCatching {
                node.javaClass.methods.firstOrNull { it.name in setOf("resolveBinding", "resolveTypeBinding") && it.parameterCount == 0 }
                    ?.invoke(node)
            }.getOrNull() ?: return null
            return descriptorFromBinding(binding)
        }

        private fun typeBindingInternalName(node: Any): String? {
            val binding = runCatching {
                node.javaClass.methods.firstOrNull { it.name == "resolveTypeBinding" && it.parameterCount == 0 }
                    ?.invoke(node)
            }.getOrNull() ?: return null
            return bindingBinaryName(binding)?.replace('.', '/')
        }

        private fun descriptorFromBinding(binding: Any): String? {
            val isPrimitive = bool(binding, "isPrimitive")
            if (isPrimitive) {
                return when (string(binding, "getName")) {
                    "void" -> "V"
                    "boolean" -> "Z"
                    "byte" -> "B"
                    "char" -> "C"
                    "short" -> "S"
                    "int" -> "I"
                    "long" -> "J"
                    "float" -> "F"
                    "double" -> "D"
                    else -> null
                }
            }
            if (bool(binding, "isArray")) {
                val component = call(binding, "getComponentType") ?: return null
                val dims = int(binding, "getDimensions") ?: 1
                return "[".repeat(dims) + descriptorFromBinding(component)
            }
            val erasure = call(binding, "getErasure") ?: binding
            val binaryName = bindingBinaryName(erasure) ?: return null
            return "L${binaryName.replace('.', '/')};"
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
            runCatching { node.javaClass.getMethod("getStructuralProperty", property.javaClass).invoke(node, property) }
                .getOrNull()

        private fun listProperty(node: Any?, name: String): List<Any> {
            if (node == null) return emptyList()
            return runCatching {
                val value = node.javaClass.getMethod(name).invoke(node)
                @Suppress("UNCHECKED_CAST")
                value as? List<Any>
            }.getOrNull().orEmpty()
        }

        private fun call(node: Any, name: String): Any? =
            runCatching { node.javaClass.methods.firstOrNull { it.name == name && it.parameterCount == 0 }?.invoke(node) }
                .getOrNull()

        private fun string(node: Any, name: String): String? =
            call(node, name) as? String

        private fun int(node: Any, name: String): Int? =
            call(node, name) as? Int

        private fun bool(node: Any, name: String): Boolean =
            call(node, name) as? Boolean ?: false
    }
}
