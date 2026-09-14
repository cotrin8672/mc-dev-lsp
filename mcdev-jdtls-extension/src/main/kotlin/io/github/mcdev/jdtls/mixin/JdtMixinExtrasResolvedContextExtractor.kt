package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixinextras.DefinitionAnnotationParseIssue
import io.github.mcdev.core.mixinextras.ExpressionContext
import io.github.mcdev.core.mixinextras.HandlerParameterSugarSpec
import io.github.mcdev.core.mixinextras.MixinExtrasDefinition
import io.github.mcdev.core.mixinextras.MixinExtrasDefinitionIndex
import io.github.mcdev.core.mixinextras.MixinExtrasExpression
import io.github.mcdev.core.mixinextras.MixinExtrasExpressionIndex
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext

sealed interface JdtMixinExtrasResolvedContextResult {
    data object NotMixinExtrasHandler : JdtMixinExtrasResolvedContextResult

    data class Resolved(
        val context: ResolvedMixinExtrasContext,
    ) : JdtMixinExtrasResolvedContextResult

    data class Unavailable(
        val reason: String,
    ) : JdtMixinExtrasResolvedContextResult
}

internal object JdtMixinExtrasResolvedContextExtractor {
    private val HANDLER_ANNOTATION_FQNS = setOf(
        "com.llamalad7.mixinextras.injector.ModifyExpressionValue",
        "com.llamalad7.mixinextras.injector.ModifyReturnValue",
        "com.llamalad7.mixinextras.injector.ModifyReceiver",
        "com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation",
        "com.llamalad7.mixinextras.injector.WrapWithCondition",
        "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
        "com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod",
        "org.spongepowered.asm.mixin.injection.Inject",
        "org.spongepowered.asm.mixin.injection.Redirect",
        "org.spongepowered.asm.mixin.injection.ModifyArg",
        "org.spongepowered.asm.mixin.injection.ModifyArgs",
        "org.spongepowered.asm.mixin.injection.ModifyVariable",
        "org.spongepowered.asm.mixin.injection.ModifyConstant",
    )

    private const val EXPRESSION_FQN = "com.llamalad7.mixinextras.expression.Expression"
    private const val EXPRESSIONS_FQN = "com.llamalad7.mixinextras.expression.Expressions"
    private const val DEFINITION_FQN = "com.llamalad7.mixinextras.expression.Definition"
    private const val DEFINITIONS_FQN = "com.llamalad7.mixinextras.expression.Definitions"
    private const val LOCAL_FQN = "com.llamalad7.mixinextras.sugar.Local"

    private val OFFICIAL_HANDLER_SIMPLE_NAMES = HANDLER_ANNOTATION_FQNS.map { it.substringAfterLast('.') }.toSet()

    private val OFFICIAL_CONTEXT_SIMPLE_NAMES = setOf(
        "Expression",
        "Expressions",
        "Definition",
        "Definitions",
        "Local",
    )

    fun extract(
        methodDeclaration: Any,
        handlerRange: McTextRange,
        source: String? = null,
    ): JdtMixinExtrasResolvedContextResult {
        val annotations = listProperty(methodDeclaration, "modifiers")
            .filter { it.javaClass.simpleName.endsWith("Annotation") }

        var hasResolvedOfficialHandler = false
        for (annotation in annotations) {
            val simpleName = annotationTypeSimpleName(annotation)
            if (simpleName !in OFFICIAL_HANDLER_SIMPLE_NAMES) {
                continue
            }
            when (val fqn = annotationTypeFqn(annotation)) {
                null -> return unavailable("unresolved handler annotation binding")
                in HANDLER_ANNOTATION_FQNS -> {
                    if (isRecoveredOrMalformed(annotation)) {
                        return unavailable("recovered or malformed handler annotation")
                    }
                    hasResolvedOfficialHandler = true
                }
                else -> Unit
            }
        }
        if (!hasResolvedOfficialHandler) {
            return JdtMixinExtrasResolvedContextResult.NotMixinExtrasHandler
        }

        val expressions = mutableListOf<MixinExtrasExpression>()
        val definitions = mutableListOf<MixinExtrasDefinition>()

        for (annotation in annotations) {
            when (val fqn = annotationTypeFqn(annotation)) {
                EXPRESSION_FQN -> {
                    val parsed = parseExpressionAnnotation(annotation)
                        ?: return unavailable("unresolved @Expression member")
                    expressions += parsed
                }
                EXPRESSIONS_FQN -> {
                    val parsed = parseExpressionsAnnotation(annotation)
                        ?: return unavailable("unresolved @Expressions member")
                    expressions += parsed
                }
                DEFINITION_FQN -> {
                    definitions += parseDefinitionAnnotation(annotation, source)
                }
                DEFINITIONS_FQN -> {
                    definitions += parseDefinitionsAnnotation(annotation, source)
                }
                null -> {
                    if (annotationTypeSimpleName(annotation) in OFFICIAL_CONTEXT_SIMPLE_NAMES) {
                        return unavailable("unresolved context annotation binding")
                    }
                }
                else -> Unit
            }
        }

        return JdtMixinExtrasResolvedContextResult.Resolved(
            ResolvedMixinExtrasContext(
                handlerRange = handlerRange,
                context = ExpressionContext(
                    expressionIndex = MixinExtrasExpressionIndex(expressions),
                    definitionIndex = MixinExtrasDefinitionIndex(definitions),
                ),
            ),
        )
    }

    private fun annotationTypeSimpleName(annotation: Any): String =
        call(annotation, "getTypeName")?.toString()?.substringAfterLast('.') ?: ""

    private fun parseExpressionAnnotation(annotation: Any): MixinExtrasExpression? {
        if (!isUsableAnnotation(annotation)) {
            return null
        }
        val members = memberMap(annotation) ?: return null
        val id = when {
            "id" in members -> JdtAnnotationConstantExtractor.constantString(members.getValue("id"))
                ?: return null
            else -> ""
        }
        val valueNode = members["value"] ?: return null
        val values = JdtAnnotationConstantExtractor.stringArray(valueNode) ?: return null
        return MixinExtrasExpression(id = id, values = values)
    }

    private fun parseExpressionsAnnotation(annotation: Any): List<MixinExtrasExpression>? {
        if (!isUsableAnnotation(annotation)) {
            return null
        }
        val valueNode = memberValue(annotation) ?: return null
        val nested = JdtAnnotationConstantExtractor.nestedAnnotationArray(valueNode) ?: return null
        val parsed = ArrayList<MixinExtrasExpression>(nested.size)
        for (nestedAnnotation in nested) {
            when (val fqn = annotationTypeFqn(nestedAnnotation)) {
                EXPRESSION_FQN -> parsed += parseExpressionAnnotation(nestedAnnotation) ?: return null
                null -> {
                    if (annotationTypeSimpleName(nestedAnnotation) == "Expression") {
                        return null
                    }
                }
                else -> Unit
            }
        }
        return parsed
    }

    private fun parseDefinitionsAnnotation(annotation: Any, source: String?): List<MixinExtrasDefinition> {
        val annotationBodyRange = definitionBodyRange(annotation, source)
        if (!isUsableAnnotation(annotation)) {
            return listOf(malformedDefinition("unresolved @Definitions member", annotationBodyRange))
        }
        val valueNode = memberValue(annotation)
            ?: return listOf(malformedDefinition("unresolved @Definitions member", annotationBodyRange))
        val nested = nestedDefinitionAnnotations(valueNode)
            ?: return listOf(malformedDefinition("unresolved @Definitions member", annotationBodyRange))
        val parsed = ArrayList<MixinExtrasDefinition>(nested.size)
        for (nestedAnnotation in nested) {
            if (!isAnnotationNode(nestedAnnotation)) {
                parsed += malformedDefinition(
                    "expected @Definition annotation",
                    astNodeRange(nestedAnnotation),
                )
                continue
            }
            when (val fqn = annotationTypeFqn(nestedAnnotation)) {
                DEFINITION_FQN -> parsed += parseDefinitionAnnotation(nestedAnnotation, source)
                null -> {
                    if (annotationTypeSimpleName(nestedAnnotation) == "Definition") {
                        parsed += malformedDefinition(
                            "unresolved @Definition member",
                            definitionBodyRange(nestedAnnotation, source),
                        )
                    }
                }
                else -> Unit
            }
        }
        return parsed
    }

    private fun parseDefinitionAnnotation(annotation: Any, source: String?): MixinExtrasDefinition {
        val annotationBodyRange = definitionBodyRange(annotation, source)
        if (!isUsableAnnotation(annotation)) {
            return malformedDefinition("unresolved @Definition member", annotationBodyRange)
        }
        val members = memberMap(annotation)
            ?: return malformedDefinition("unresolved @Definition member", annotationBodyRange)
        val issues = mutableListOf<DefinitionAnnotationParseIssue>()
        val id = members["id"]?.let { node ->
            JdtAnnotationConstantExtractor.constantString(node) ?: run {
                issues += parseIssue("id", node, "expected a string literal", annotationBodyRange)
                null
            }
        }
        val methodReferences = extractOptionalStringArray(members, "method") { node ->
            issues += parseIssue("method", node, "expected a string or string array", annotationBodyRange)
        } ?: emptyList()
        val fieldReferences = extractOptionalStringArray(members, "field") { node ->
            issues += parseIssue("field", node, "expected a string or string array", annotationBodyRange)
        } ?: emptyList()
        val classLiteralTypeNames = extractOptionalTypeNames(members, "type") { node ->
            issues += parseIssue("type", node, "expected a class literal or class literal array", annotationBodyRange)
        } ?: emptyList()
        val localSpecs = extractOptionalLocalSpecs(members, "local") { node ->
            issues += parseIssue("local", node, "expected @Local or @Local array", annotationBodyRange)
        } ?: emptyList()
        val remap = members["remap"]?.let { node ->
            JdtAnnotationConstantExtractor.constantBoolean(node) ?: run {
                issues += parseIssue("remap", node, "expected a boolean literal", annotationBodyRange)
                null
            }
        }
        return MixinExtrasDefinition(
            id = id,
            rawMethodReferences = methodReferences,
            rawFieldReferences = fieldReferences,
            classLiteralTypeNames = classLiteralTypeNames,
            localSpecs = localSpecs,
            remap = remap,
        ).withSemanticSourceRange(annotationBodyRange).withParseIssues(issues)
    }

    private fun parseLocalAnnotation(annotation: Any): HandlerParameterSugarSpec.Local? {
        if (!isUsableAnnotation(annotation)) {
            return null
        }
        if (annotationTypeFqn(annotation) != LOCAL_FQN) {
            return null
        }
        val members = memberMap(annotation) ?: return null
        val argsOnly = when {
            "argsOnly" in members -> JdtAnnotationConstantExtractor.constantBoolean(members.getValue("argsOnly"))
                ?: return null
            else -> false
        }
        val index = when {
            "index" in members -> {
                val constant = JdtAnnotationConstantExtractor.constantInt(members.getValue("index")) ?: return null
                if (constant == -1) null else constant
            }
            else -> null
        }
        val ordinal = when {
            "ordinal" in members -> {
                val constant = JdtAnnotationConstantExtractor.constantInt(members.getValue("ordinal")) ?: return null
                if (constant == -1) null else constant
            }
            else -> null
        }
        val names = when {
            "name" in members -> {
                JdtAnnotationConstantExtractor.stringArray(members.getValue("name"))?.toSet() ?: return null
            }
            else -> emptySet()
        }
        val print = when {
            "print" in members -> JdtAnnotationConstantExtractor.constantBoolean(members.getValue("print"))
                ?: return null
            else -> false
        }
        val typeClassName = when {
            "type" in members -> JdtAnnotationConstantExtractor.typeLiteralSourceName(members.getValue("type"))
                ?: return null
            else -> null
        }
        return HandlerParameterSugarSpec.Local(
            argsOnly = argsOnly,
            index = index,
            ordinal = ordinal,
            names = names,
            print = print,
            typeClassName = typeClassName,
        )
    }

    private fun extractOptionalStringArray(
        members: Map<String, Any>,
        key: String,
        onFailure: (Any) -> Unit,
    ): List<String>? {
        val node = members[key] ?: return emptyList()
        return JdtAnnotationConstantExtractor.stringArray(node) ?: run {
            onFailure(node)
            null
        }
    }

    private fun extractOptionalTypeNames(
        members: Map<String, Any>,
        key: String,
        onFailure: (Any) -> Unit,
    ): List<String>? {
        val node = members[key] ?: return emptyList()
        return JdtAnnotationConstantExtractor.typeArraySourceNames(node) ?: run {
            onFailure(node)
            null
        }
    }

    private fun extractOptionalLocalSpecs(
        members: Map<String, Any>,
        key: String,
        onFailure: (Any) -> Unit,
    ): List<HandlerParameterSugarSpec.Local>? {
        val node = members[key] ?: return emptyList()
        val nested = JdtAnnotationConstantExtractor.nestedAnnotationArray(node) ?: run {
            onFailure(node)
            return null
        }
        val parsed = ArrayList<HandlerParameterSugarSpec.Local>(nested.size)
        for (nestedAnnotation in nested) {
            when (val fqn = annotationTypeFqn(nestedAnnotation)) {
                LOCAL_FQN -> {
                    val local = parseLocalAnnotation(nestedAnnotation)
                    if (local == null) {
                        onFailure(node)
                        return null
                    }
                    parsed += local
                }
                null -> {
                    if (annotationTypeSimpleName(nestedAnnotation) == "Local") {
                        onFailure(node)
                        return null
                    }
                }
                else -> Unit
            }
        }
        return parsed
    }

    private fun nestedDefinitionAnnotations(node: Any): List<Any>? {
        if (isRecoveredOrMalformed(node)) return null
        return when {
            isAnnotationNode(node) -> listOf(node)
            node.javaClass.simpleName == "ArrayInitializer" -> listProperty(node, "expressions")
            else -> listOf(node)
        }
    }

    private fun malformedDefinition(
        message: String,
        sourceRange: IntRange? = null,
    ): MixinExtrasDefinition =
        MixinExtrasDefinition().withParseIssues(
            listOf(
                DefinitionAnnotationParseIssue(
                    attribute = "id",
                    rawValue = "",
                    message = message,
                    bodyRange = sourceRange?.let { 0 until it.length() } ?: (0 until 0),
                ),
            ),
        ).withSemanticSourceRange(sourceRange)

    private fun parseIssue(
        attribute: String,
        node: Any,
        message: String,
        definitionBodyRange: IntRange?,
    ): DefinitionAnnotationParseIssue {
        val bodyRange = definitionBodyRange?.let { body ->
            astNodeRange(node)?.let { relativeRange(it, body) }
        } ?: (0 until 0)
        return DefinitionAnnotationParseIssue(
            attribute = attribute,
            rawValue = node.toString(),
            message = message,
            bodyRange = bodyRange,
        )
    }

    private fun definitionBodyRange(annotation: Any, source: String?): IntRange? {
        if (source == null) return null
        val annotationRange = astNodeRange(annotation) ?: return null
        val annotationStart = annotationRange.first
        val annotationEnd = (annotationRange.last + 1).coerceAtMost(source.length)
        if (annotationStart !in 0 until source.length || annotationEnd <= annotationStart) return null
        val openParen = source.indexOf('(', annotationStart)
        if (openParen < annotationStart || openParen >= annotationEnd) return null
        val closeParen = if (source.getOrNull(annotationEnd - 1) == ')') annotationEnd - 1 else annotationEnd
        return (openParen + 1).coerceAtMost(closeParen) until closeParen
    }

    private fun astNodeRange(node: Any): IntRange? {
        val start = int(node, "getStartPosition") ?: return null
        val length = int(node, "getLength") ?: return null
        if (start < 0 || length <= 0) return null
        return start until (start + length)
    }

    private fun relativeRange(range: IntRange, body: IntRange): IntRange {
        val bodyStart = body.first
        val bodyEnd = body.last + 1
        val start = maxOf(range.first, bodyStart).coerceAtMost(bodyEnd)
        val end = minOf(range.last + 1, bodyEnd).coerceAtLeast(start)
        return (start - bodyStart) until (end - bodyStart)
    }

    private fun IntRange.length(): Int = if (isEmpty()) 0 else last - first + 1

    private fun isAnnotationNode(node: Any?): Boolean =
        node?.javaClass?.simpleName?.endsWith("Annotation") == true

    private fun memberValue(annotation: Any): Any? =
        when (annotation.javaClass.simpleName) {
            "MarkerAnnotation" -> null
            "SingleMemberAnnotation" -> call(annotation, "getValue")
            "NormalAnnotation" -> memberMap(annotation)?.get("value")
            else -> null
        }

    private fun memberMap(annotation: Any): Map<String, Any>? {
        return when (annotation.javaClass.simpleName) {
            "MarkerAnnotation" -> emptyMap()
            "SingleMemberAnnotation" -> {
                val value = call(annotation, "getValue") ?: return null
                mapOf("value" to value)
            }
            "NormalAnnotation" -> {
                val result = LinkedHashMap<String, Any>()
                for (pair in listProperty(annotation, "values")) {
                    val name = nodeName(pair) ?: return null
                    val value = call(pair, "getValue") ?: return null
                    if (name in result) {
                        return null
                    }
                    result[name] = value
                }
                result
            }
            else -> emptyMap()
        }
    }

    private fun isUsableAnnotation(annotation: Any): Boolean =
        !isRecoveredOrMalformed(annotation)

    private fun isRecoveredOrMalformed(node: Any): Boolean {
        if (hasMethod(node, "getFlags")) {
            val flags = int(node, "getFlags") ?: return false
            val recoveredFlag = astNodeFlag(node, "RECOVERED")
            val malformedFlag = astNodeFlag(node, "MALFORMED")
            if (recoveredFlag != null || malformedFlag != null) {
                if (recoveredFlag != null && flags and recoveredFlag != 0) {
                    return true
                }
                if (malformedFlag != null && flags and malformedFlag != 0) {
                    return true
                }
                return false
            }
        }
        return bool(node, "isRecovered") || bool(node, "isMalformed")
    }

    private fun astNodeFlag(node: Any, name: String): Int? {
        var clazz: Class<*>? = node.javaClass
        while (clazz != null) {
            val field = runCatching { clazz.getField(name) }.getOrNull()
            if (field != null && field.type == Int::class.javaPrimitiveType) {
                return runCatching { field.getInt(null) }.getOrNull()
            }
            clazz = clazz.superclass
        }
        val astNodeClass = runCatching { Class.forName("org.eclipse.jdt.core.dom.ASTNode") }.getOrNull()
        if (astNodeClass != null) {
            val field = runCatching { astNodeClass.getField(name) }.getOrNull()
            if (field != null && field.type == Int::class.javaPrimitiveType) {
                return runCatching { field.getInt(null) }.getOrNull()
            }
        }
        return null
    }

    private fun hasMethod(node: Any, name: String): Boolean =
        node.javaClass.methods.any { it.name == name && it.parameterCount == 0 }

    private fun annotationTypeFqn(annotation: Any): String? {
        val annotationBinding = call(annotation, "resolveAnnotationBinding") ?: return null
        val binding = call(annotationBinding, "getAnnotationType") ?: return null
        return qualifiedNameFromBinding(binding)
    }

    private fun qualifiedNameFromBinding(binding: Any): String? =
        string(binding, "getQualifiedName")
            ?: string(binding, "getBinaryName")

    private fun unavailable(reason: String): JdtMixinExtrasResolvedContextResult.Unavailable =
        JdtMixinExtrasResolvedContextResult.Unavailable(reason)

    private fun listProperty(node: Any?, name: String): List<Any> {
        if (node == null) {
            return emptyList()
        }
        return runCatching {
            val value = node.javaClass.getMethod(name).apply { trySetAccessible() }.invoke(node)
            @Suppress("UNCHECKED_CAST")
            value as? List<Any>
        }.onFailure { JdtMixinSemanticModelParser.throwIfJdtAbort(it) }.getOrNull().orEmpty()
    }

    private fun nodeName(node: Any?): String? =
        node?.let { call(it, "getName")?.toString() }

    private fun call(node: Any, name: String): Any? =
        runCatching {
            node.javaClass.methods
                .firstOrNull { it.name == name && it.parameterCount == 0 }
                ?.apply { trySetAccessible() }
                ?.invoke(node)
        }.onFailure { JdtMixinSemanticModelParser.throwIfJdtAbort(it) }.getOrNull()

    private fun string(node: Any, name: String): String? =
        call(node, name) as? String

    private fun int(node: Any, name: String): Int? =
        call(node, name) as? Int

    private fun bool(node: Any, name: String): Boolean =
        call(node, name) as? Boolean ?: false
}
