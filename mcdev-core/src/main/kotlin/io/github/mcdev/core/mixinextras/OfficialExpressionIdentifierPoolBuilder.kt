package io.github.mcdev.core.mixinextras

import com.llamalad7.mixinextras.expression.impl.pool.IdentifierPool
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseFieldSelector
import io.github.mcdev.core.descriptor.parseMethodSelector

data class OfficialExpressionIdentifierPoolBuildIssue(
    val definitionId: String?,
    val attribute: String,
    val rawValue: String,
    val message: String,
    val sourceRange: IntRange? = null,
)

data class OfficialExpressionIdentifierPoolBuildResult(
    val pool: OfficialExpressionIdentifierPool,
    val issues: List<OfficialExpressionIdentifierPoolBuildIssue>,
)

object OfficialExpressionIdentifierPoolBuilder {
    private const val METHOD_ATTRIBUTE = "method"
    private const val FIELD_ATTRIBUTE = "field"
    private const val TYPE_ATTRIBUTE = "type"
    private const val LOCAL_ATTRIBUTE = "local"

    fun build(
        index: MixinExtrasDefinitionIndex,
        typeNameResolver: ClassLiteralTypeNameResolver = ClassLiteralTypeNameResolver.FAIL_CLOSED,
    ): OfficialExpressionIdentifierPoolBuildResult {
        val delegate = IdentifierPool()
        val issues = mutableListOf<OfficialExpressionIdentifierPoolBuildIssue>()
        val locals = mutableListOf<ExpressionLocalDefinition>()

        for (definition in index.definitions) {
            for (parseIssue in definition.parseIssues) {
                issues += OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = definition.id,
                    attribute = parseIssue.attribute,
                    rawValue = parseIssue.rawValue,
                    message = parseIssue.message,
                    sourceRange = definition.sourceRange?.let { body ->
                        body.first + parseIssue.bodyRange.first until body.first + parseIssue.bodyRange.last + 1
                    },
                )
            }
            if (definition.id == null && definition.parseIssues.none { it.attribute == "id" } &&
                definition.rawMethodReferences.isEmpty() &&
                definition.rawFieldReferences.isEmpty() && definition.classLiteralTypeNames.isEmpty() &&
                definition.localSpecs.isEmpty()) {
                issues += OfficialExpressionIdentifierPoolBuildIssue(
                    definitionId = null,
                    attribute = "id",
                    rawValue = "",
                    message = "definition requires a string id",
                    sourceRange = definition.sourceRange,
                )
            }
            if (definition.rawMethodReferences.isNotEmpty()) {
                val id = definition.id
                if (id == null) {
                    for (raw in definition.rawMethodReferences) {
                        issues += OfficialExpressionIdentifierPoolBuildIssue(
                            definitionId = null,
                            attribute = METHOD_ATTRIBUTE,
                            rawValue = raw,
                            message = "method definition requires id",
                            sourceRange = definition.sourceRange,
                        )
                    }
                } else {
                    for (raw in definition.rawMethodReferences) {
                        when (val parsed = parseMethodSelector(raw)) {
                            is DescriptorParseResult.Success ->
                                delegate.addMember(id, MethodSelectorMemberDefinition(parsed.value))
                            is DescriptorParseResult.Failure ->
                                issues += OfficialExpressionIdentifierPoolBuildIssue(
                                    definitionId = id,
                                    attribute = METHOD_ATTRIBUTE,
                                    rawValue = raw,
                                    message = parsed.error.message,
                                    sourceRange = definition.sourceRange,
                                )
                        }
                    }
                }
            }

            if (definition.rawFieldReferences.isNotEmpty()) {
                val id = definition.id
                if (id == null) {
                    for (raw in definition.rawFieldReferences) {
                        issues += OfficialExpressionIdentifierPoolBuildIssue(
                            definitionId = null,
                            attribute = FIELD_ATTRIBUTE,
                            rawValue = raw,
                            message = "field definition requires id",
                            sourceRange = definition.sourceRange,
                        )
                    }
                } else {
                    for (raw in definition.rawFieldReferences) {
                        when (val parsed = parseFieldSelector(raw)) {
                            is DescriptorParseResult.Success ->
                                delegate.addMember(id, FieldSelectorMemberDefinition(parsed.value))
                            is DescriptorParseResult.Failure ->
                                issues += OfficialExpressionIdentifierPoolBuildIssue(
                                    definitionId = id,
                                    attribute = FIELD_ATTRIBUTE,
                                    rawValue = raw,
                                    message = parsed.error.message,
                                    sourceRange = definition.sourceRange,
                                )
                        }
                    }
                }
            }

            if (definition.classLiteralTypeNames.isNotEmpty()) {
                val id = definition.id
                if (id == null) {
                    for (raw in definition.classLiteralTypeNames) {
                        issues += OfficialExpressionIdentifierPoolBuildIssue(
                            definitionId = null,
                            attribute = TYPE_ATTRIBUTE,
                            rawValue = raw,
                            message = "type definition requires id",
                            sourceRange = definition.sourceRange,
                        )
                    }
                } else {
                    for (raw in definition.classLiteralTypeNames) {
                        val resolved = typeNameResolver.resolve(raw)
                        when {
                            resolved == null ->
                                issues += OfficialExpressionIdentifierPoolBuildIssue(
                                    definitionId = id,
                                    attribute = TYPE_ATTRIBUTE,
                                    rawValue = raw,
                                    message = classLiteralTypeResolutionFailureMessage(raw),
                                    sourceRange = definition.sourceRange,
                                )
                            !isValidClassLiteralResolvedType(resolved) ->
                                issues += OfficialExpressionIdentifierPoolBuildIssue(
                                    definitionId = id,
                                    attribute = TYPE_ATTRIBUTE,
                                    rawValue = raw,
                                    message = "invalid resolved type",
                                    sourceRange = definition.sourceRange,
                                )
                            else -> delegate.addType(id, ExactTypeDefinition(resolved))
                        }
                    }
                }
            }

            if (definition.localSpecs.isNotEmpty()) {
                val id = definition.id
                if (id == null) {
                    for (spec in definition.localSpecs) {
                        issues += OfficialExpressionIdentifierPoolBuildIssue(
                            definitionId = null,
                            attribute = LOCAL_ATTRIBUTE,
                            rawValue = spec.toString(),
                            message = "local definition requires id",
                            sourceRange = definition.sourceRange,
                        )
                    }
                } else {
                    for (spec in definition.localSpecs) {
                        val type = spec.typeClassName?.let(typeNameResolver::resolve)
                        if (type != null && type.sort != org.objectweb.asm.Type.VOID &&
                            isValidClassLiteralResolvedType(type)) {
                            locals += ExpressionLocalDefinition(id, spec, type.descriptor)
                            // Register the identifier now; method-specific matching binds its live locals later.
                            delegate.addMember(id, LocalIndexMemberDefinition(-1))
                        } else {
                            issues += OfficialExpressionIdentifierPoolBuildIssue(
                                definitionId = id,
                                attribute = LOCAL_ATTRIBUTE,
                                rawValue = spec.toString(),
                                message = "local definition requires a resolvable non-void type",
                                sourceRange = definition.sourceRange,
                            )
                        }
                    }
                }
            }
        }

        return OfficialExpressionIdentifierPoolBuildResult(
            pool = OfficialExpressionIdentifierPool(delegate, locals),
            issues = issues,
        )
    }
}
