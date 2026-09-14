package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.bytecode.ConstantValue
import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.DescriptorRenderer
import io.github.mcdev.core.descriptor.MemberTarget
import io.github.mcdev.core.descriptor.MemberTargetParser
import io.github.mcdev.core.descriptor.Pattern
import io.github.mcdev.core.descriptor.parseFieldSelector
import io.github.mcdev.core.descriptor.parseMethodSelector
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtTargetCompletionService
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.MethodIndexEntry

class InjectionPointOccurrenceResolver(
    private val bytecodeIndex: BytecodeIndex,
) {
    private val targetFormatter = AtTargetCompletionService()

    fun resolve(
        owner: String,
        targetMethod: MethodIndexEntry,
        site: MixinExtrasAnnotationSite,
        expressionInstructionIndices: Set<Int>? = null,
    ): Set<Int>? {
        val (rawAtValue, specifier) = parseAtValue(site.atValue ?: return null) ?: return null
        val atValue = rawAtValue.uppercase()
        return when (atValue) {
            "MIXINEXTRAS:EXPRESSION" -> {
                val indices = expressionInstructionIndices ?: return null
                val selected = selectIndexSpecifier(indices, specifier) ?: return null
                applyShift(selected, site.atShift)
            }
            "HEAD" -> {
                val selected = selectIndexSpecifier(setOf(0), specifier) ?: return null
                applyShift(selected, site.atShift)
            }
            "TAIL" -> resolveTail(owner, targetMethod, site, specifier)
            "INVOKE",
            "FIELD",
            "NEW",
            "RETURN",
            "CONSTANT",
            -> resolveAtTargetCandidates(owner, targetMethod, site, atValue, specifier)
            else -> null
        }
    }

    private fun parseAtValue(value: String): Pair<String, String?>? {
        if (value.equals("MIXINEXTRAS:EXPRESSION", ignoreCase = true)) {
            return value to null
        }
        val separator = value.lastIndexOf(':')
        if (separator < 0) {
            return value to null
        }
        val base = value.substring(0, separator)
        val suffix = value.substring(separator + 1)
        if (base.isEmpty() || suffix.isEmpty()) {
            return null
        }
        if (suffix !in setOf("FIRST", "LAST", "ONE", "ALL", "DEFAULT")) {
            return null
        }
        return base to suffix
    }

    private fun resolveTail(
        owner: String,
        targetMethod: MethodIndexEntry,
        site: MixinExtrasAnnotationSite,
        specifier: String?,
    ): Set<Int>? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName = owner,
            methodName = targetMethod.name,
            methodDescriptor = targetMethod.descriptor,
            atValue = "RETURN",
        )
        val maxIndex = candidates
            .asSequence()
            .map { it.instructionOccurrenceIndex }
            .filter { it >= 0 }
            .maxOrNull()
            ?: return null
        val selected = selectIndexSpecifier(setOf(maxIndex), specifier) ?: return null
        return applyShift(selected, site.atShift)
    }

    private fun resolveAtTargetCandidates(
        owner: String,
        targetMethod: MethodIndexEntry,
        site: MixinExtrasAnnotationSite,
        atValue: String,
        specifier: String?,
    ): Set<Int>? {
        val candidates = bytecodeIndex.getAtTargetCandidates(
            ownerInternalName = owner,
            methodName = targetMethod.name,
            methodDescriptor = targetMethod.descriptor,
            atValue = atValue,
        )
        val filtered = when (atValue) {
            "CONSTANT" -> filterConstantCandidates(candidates, site) ?: return null
            else -> filterByAtTarget(candidates, site.atTarget, atValue) ?: return null
        }
        val selectedIndices = selectInstructionOccurrenceIndices(filtered, site.atOrdinal, specifier) ?: return null
        return applyShift(selectedIndices, site.atShift)
    }

    private fun selectInstructionOccurrenceIndices(
        candidates: List<AtTargetCandidate>,
        atOrdinal: Int?,
        specifier: String?,
    ): Set<Int>? {
        if (atOrdinal != null && candidates.any { it.instructionOccurrenceIndex < 0 }) {
            return null
        }
        val ordinalSelected = if (atOrdinal == null) {
            candidates
        } else {
            candidates
                .sortedBy { it.instructionOccurrenceIndex }
                .getOrNull(atOrdinal)
                ?.let(::listOf)
                ?: emptyList()
        }
        val specified = selectAtSpecifier(ordinalSelected, specifier) ?: return null
        return specified.map { it.instructionOccurrenceIndex }.toSet()
    }

    private fun selectAtSpecifier(
        candidates: List<AtTargetCandidate>,
        specifier: String?,
    ): List<AtTargetCandidate>? = when (specifier) {
        null,
        "ALL",
        "DEFAULT",
        -> candidates
        "FIRST" -> candidates.minByOrNull { it.instructionOccurrenceIndex }?.let(::listOf) ?: emptyList()
        "LAST" -> candidates.maxByOrNull { it.instructionOccurrenceIndex }?.let(::listOf) ?: emptyList()
        "ONE" -> if (candidates.size == 1) candidates else null
        else -> null
    }

    private fun selectIndexSpecifier(indices: Set<Int>, specifier: String?): Set<Int>? {
        if (specifier == null || specifier == "ALL" || specifier == "DEFAULT") {
            return indices
        }
        val sorted = indices.sorted()
        return when (specifier) {
            "FIRST" -> sorted.firstOrNull()?.let(::setOf) ?: emptySet()
            "LAST" -> sorted.lastOrNull()?.let(::setOf) ?: emptySet()
            "ONE" -> if (indices.size == 1) indices else null
            else -> null
        }
    }

    private fun filterByAtTarget(
        candidates: List<AtTargetCandidate>,
        atTarget: String?,
        atValue: String,
    ): List<AtTargetCandidate>? {
        if (atTarget.isNullOrBlank()) {
            return candidates
        }
        val trimmed = atTarget.trim()
        if (atValue == "RETURN") {
            return if (trimmed != "RETURN") {
                emptyList()
            } else {
                candidates.filter { it.kind == AtTargetKind.RETURN }
            }
        }
        if (isBroadSelector(trimmed)) {
            return null
        }
        return when (atValue) {
            "INVOKE", "FIELD" -> filterMemberTargetCandidates(candidates, trimmed, atValue)
            "NEW" -> filterNewCandidates(candidates, trimmed) ?: return null
            else -> candidates.filter { targetFormatter.formatTarget(it) == trimmed }
        }
    }

    private fun filterMemberTargetCandidates(
        candidates: List<AtTargetCandidate>,
        atTarget: String,
        atValue: String,
    ): List<AtTargetCandidate> {
        val expectedKind = when (atValue) {
            "INVOKE" -> AtTargetKind.INVOKE
            "FIELD" -> AtTargetKind.FIELD
            else -> return emptyList()
        }
        val byFormat = candidates.filter {
            it.kind == expectedKind && targetFormatter.formatTarget(it) == atTarget
        }
        if (byFormat.isNotEmpty()) {
            return byFormat
        }
        return when (val parsed = MemberTargetParser.parse(atTarget)) {
            is DescriptorParseResult.Success -> when (val target = parsed.value) {
                is MemberTarget.Method -> candidates.filter {
                    it.kind == AtTargetKind.INVOKE &&
                        it.owner == target.owner &&
                        it.name == target.name &&
                        it.descriptor == DescriptorRenderer.toDescriptor(target.descriptor)
                }
                is MemberTarget.Field -> candidates.filter {
                    it.kind == AtTargetKind.FIELD &&
                        it.owner == target.owner &&
                        it.name == target.name &&
                        it.descriptor == DescriptorRenderer.toDescriptor(target.descriptor)
                }
            }
            is DescriptorParseResult.Failure -> emptyList()
        }
    }

    private fun filterNewCandidates(
        candidates: List<AtTargetCandidate>,
        atTarget: String,
    ): List<AtTargetCandidate>? {
        val byFormat = candidates.filter {
            it.kind == AtTargetKind.NEW && targetFormatter.formatTarget(it) == atTarget
        }
        if (byFormat.isNotEmpty()) {
            return byFormat
        }
        return when (val parsed = parseMethodSelector(atTarget)) {
            is DescriptorParseResult.Success -> {
                val selector = parsed.value
                if (hasAnyPattern(selector)) {
                    null
                } else {
                    candidates.filter { candidate ->
                        candidate.kind == AtTargetKind.NEW && matchesExactNewSelector(candidate, selector)
                    }
                }
            }
            is DescriptorParseResult.Failure -> emptyList()
        }
    }

    private fun filterConstantCandidates(
        candidates: List<AtTargetCandidate>,
        site: MixinExtrasAnnotationSite,
    ): List<AtTargetCandidate>? {
        val constantCandidates = candidates.filter { it.kind == AtTargetKind.CONSTANT }
        var filtered = if (site.expandZeroConditions.isNotEmpty()) {
            val conditionCandidates = constantCandidates.filter { candidate ->
                candidate.constantValue == null &&
                    candidate.conditionOpcode != null &&
                    candidate.conditionOpcode in site.expandZeroConditions
            }
            if (site.atArgs.isEmpty()) {
                conditionCandidates
            } else {
                val expectedValue = parseExpectedConstantValue(site.atArgs) ?: return null
                val argsMap = site.atArgs.associate { arg ->
                    val parts = arg.split('=', limit = 2)
                    parts[0] to parts.getOrElse(1) { "" }
                }
                if (argsMap.keys.any { it != "intValue" }) return null
                val literalCandidates = if (expectedValue == ConstantValue.IntValue(0)) {
                    constantCandidates.filter {
                        it.conditionOpcode == null && it.constantValue == expectedValue
                    }
                } else {
                    emptyList()
                }
                literalCandidates + conditionCandidates
            }
        } else if (site.atArgs.isEmpty()) {
            val handlerDescriptor = site.handlerMethod?.returnTypeDescriptor ?: return null
            constantCandidates.filter { candidate ->
                candidate.conditionOpcode == null &&
                    constantValueMatchesHandlerReturnType(candidate.constantValue, handlerDescriptor)
            }
        } else {
            if (isVoidClassSelector(site.atArgs)) {
                return emptyList()
            }
            val expectedValue = parseExpectedConstantValue(site.atArgs) ?: return null
            constantCandidates.filter {
                it.conditionOpcode == null && it.constantValue == expectedValue
            }
        }
        val atTarget = site.atTarget?.trim()
        if (!atTarget.isNullOrBlank()) {
            if (isBroadSelector(atTarget)) {
                return null
            }
            filtered = filtered.filter { targetFormatter.formatTarget(it) == atTarget }
        }
        return filtered
    }

    private fun applyShift(indices: Set<Int>, atShift: AtShiftSpec): Set<Int>? {
        if (atShift is AtShiftSpec.Unresolved) {
            return null
        }
        val shifted = indices.map { index ->
            when (atShift) {
                AtShiftSpec.Before -> index
                AtShiftSpec.After -> index + 1
                is AtShiftSpec.By -> index + atShift.offset
                AtShiftSpec.Unresolved -> return null
            }
        }
        if (shifted.any { it < 0 }) {
            return null
        }
        return shifted.toSet()
    }

    private fun isVoidClassSelector(atArgs: List<String>): Boolean {
        val argsMap = linkedMapOf<String, String>()
        for (arg in atArgs) {
            val parts = arg.split('=', limit = 2)
            argsMap[parts[0]] = parts.getOrElse(1) { "" }
        }
        if (argsMap["classValue"] != "void") {
            return false
        }
        val discriminatorKeys = setOf(
            "intValue",
            "floatValue",
            "longValue",
            "doubleValue",
            "stringValue",
            "classValue",
            "nullValue",
        )
        return argsMap.keys.none { it in discriminatorKeys && it != "classValue" }
    }

    private fun constantValueMatchesHandlerReturnType(
        value: ConstantValue?,
        handlerDescriptor: String,
    ): Boolean {
        if (value == null) {
            return false
        }
        return when (handlerDescriptor) {
            "I", "Z", "B", "C", "S" -> value is ConstantValue.IntValue
            "J" -> value is ConstantValue.LongValue
            "F" -> value is ConstantValue.FloatValue
            "D" -> value is ConstantValue.DoubleValue
            "Ljava/lang/String;" -> value is ConstantValue.StringValue
            "Ljava/lang/Class;" -> value is ConstantValue.ClassLiteral
            "Ljava/lang/Object;" -> value is ConstantValue.NullValue
            else -> false
        }
    }

    private fun parseExpectedConstantValue(atArgs: List<String>): ConstantValue? {
        if (ConstantAtArgsParser.parse(atArgs) == null) {
            return null
        }
        val argsMap = linkedMapOf<String, String>()
        for (arg in atArgs) {
            val parts = arg.split('=', limit = 2)
            argsMap[parts[0]] = parts.getOrElse(1) { "" }
        }
        argsMap["intValue"]?.toIntOrNull()?.let { return ConstantValue.IntValue(it) }
        argsMap["floatValue"]?.toFloatOrNull()?.let { return ConstantValue.FloatValue(it) }
        argsMap["longValue"]?.toLongOrNull()?.let { return ConstantValue.LongValue(it) }
        argsMap["doubleValue"]?.toDoubleOrNull()?.let { return ConstantValue.DoubleValue(it) }
        if (argsMap.containsKey("stringValue")) {
            return ConstantValue.StringValue(argsMap.getValue("stringValue"))
        }
        argsMap["classValue"]?.takeIf { it.isNotBlank() }?.let { classValue ->
            return ConstantValue.ClassLiteral(classValue.replace('.', '/'))
        }
        if (java.lang.Boolean.parseBoolean(argsMap["nullValue"])) {
            return ConstantValue.NullValue
        }
        return null
    }

    private fun isBroadSelector(atTarget: String): Boolean {
        if (atTarget == "*") {
            return true
        }
        when (val parsed = parseMethodSelector(atTarget)) {
            is DescriptorParseResult.Success -> if (hasAnyPattern(parsed.value)) return true
            is DescriptorParseResult.Failure -> Unit
        }
        when (val parsed = parseFieldSelector(atTarget)) {
            is DescriptorParseResult.Success -> if (hasAnyPattern(parsed.value)) return true
            is DescriptorParseResult.Failure -> Unit
        }
        return false
    }

    private fun hasAnyPattern(selector: io.github.mcdev.core.descriptor.MethodSelector): Boolean =
        selector.owner is Pattern.Any ||
            selector.name is Pattern.Any ||
            selector.descriptor is Pattern.Any

    private fun hasAnyPattern(selector: io.github.mcdev.core.descriptor.FieldSelector): Boolean =
        selector.owner is Pattern.Any ||
            selector.name is Pattern.Any ||
            selector.descriptor is Pattern.Any

    private fun matchesExactNewSelector(
        candidate: AtTargetCandidate,
        selector: io.github.mcdev.core.descriptor.MethodSelector,
    ): Boolean {
        val owner = (selector.owner as? Pattern.Exact)?.value ?: return false
        if (candidate.owner != owner) {
            return false
        }
        return when (val name = selector.name) {
            is Pattern.Exact ->
                name.value == "<init>" &&
                    candidate.name == "<init>" &&
                    selector.descriptor is Pattern.Exact &&
                    candidate.descriptor == DescriptorRenderer.toDescriptor(selector.descriptor.value)
            Pattern.Any ->
                selector.descriptor is Pattern.Any
        }
    }
}
