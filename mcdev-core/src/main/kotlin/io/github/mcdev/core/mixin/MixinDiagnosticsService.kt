package io.github.mcdev.core.mixin

import io.github.mcdev.core.descriptor.MemberTargetParser
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange

data class MixinDiagnosticRequest(
    val source: String,
    val documentUri: String,
    val mixinClassName: String?,
    val mixinPackage: String?,
    val mixinConfigContent: String?,
    val mixinConfigPath: String?,
)

class MixinDiagnosticsService(
    private val classIndex: ClassIndex,
    private val bytecodeIndex: BytecodeIndex,
    private val configEditor: MixinConfigEditor = MixinConfigEditor(),
    private val atTargetFormatter: AtTargetCompletionService = AtTargetCompletionService(),
    private val overwriteValidation: OverwriteValidationService = OverwriteValidationService(classIndex),
) {
    fun analyze(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        val model = MixinSemanticModelParser.parse(request.source)
        diagnostics += analyzeMixinTargets(request, model)
        diagnostics += analyzeMixinConfig(request)
        diagnostics += analyzeInjectMethods(request, model)
        diagnostics += analyzeAtTargets(request, model)
        diagnostics += analyzeOverwriteMethods(request)
        return diagnostics
    }

    private fun analyzeMixinTargets(request: MixinDiagnosticRequest, model: MixinClassModel): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        AnnotationContextExtractor.findAnnotationOffsets(request.source, MixinAnnotation.MIXIN).forEach { atOffset ->
            val end = AnnotationContextExtractor.annotationEndOffset(request.source, atOffset)
            val range = offsetRange(request.source, atOffset, end)
            results += duplicateMixinTargetDiagnostics(request.source, atOffset, range)
            for (target in model.targets) {
                if (MixinTargetResolver.resolveTarget(target.internalName, classIndex) == null) {
                    results += McDiagnostic(
                        code = MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET,
                        severity = McSeverity.ERROR,
                        message = "Unresolved @Mixin target: ${target.internalName}",
                        range = target.range,
                        metadata = mapOf("target" to target.internalName),
                    )
                }
            }
        }
        return results
    }

    private fun duplicateMixinTargetDiagnostics(
        source: String,
        mixinAtOffset: Int,
        range: McTextRange,
    ): List<McDiagnostic> {
        val rawTargets = collectRawMixinTargets(source, mixinAtOffset)
        val duplicates = rawTargets
            .map(::normalizeMixinTargetKey)
            .groupingBy { it }
            .eachCount()
            .filter { it.value > 1 }
            .keys
        return duplicates.map { duplicate ->
            McDiagnostic(
                code = MixinDiagnosticCodes.DUPLICATE_MIXIN_TARGET,
                severity = McSeverity.WARNING,
                message = "Duplicate @Mixin target entry: $duplicate",
                range = range,
                metadata = mapOf("target" to duplicate),
            )
        }
    }

    private fun collectRawMixinTargets(source: String, mixinAtOffset: Int): List<String> =
        AnnotationContextExtractor.parseMixinTargetValues(source, mixinAtOffset)

    private fun normalizeMixinTargetKey(raw: String): String =
        raw.trim().replace('.', '/').substringAfterLast('/')

    private fun analyzeMixinConfig(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val mixinName = request.mixinClassName ?: return emptyList()
        val configContent = request.mixinConfigContent ?: return emptyList()
        val config = configEditor.parse(configContent, request.mixinConfigPath.orEmpty())
        val listed = config.mixins + config.client + config.server
        if (mixinName !in listed) {
            val classDecl = Regex("""\bclass\s+(\w+)""").find(request.source)
            val range = classDecl?.let {
                offsetRange(request.source, it.range.first, it.range.last + 1)
            } ?: McTextRange(McTextPosition(0, 0), McTextPosition(0, 0))
            return listOf(
                McDiagnostic(
                    code = MixinDiagnosticCodes.MIXIN_CLASS_NOT_LISTED_IN_CONFIG,
                    severity = McSeverity.WARNING,
                    message = "Mixin class '$mixinName' is not listed in mixin config",
                    range = range,
                    metadata = mapOf(
                        "mixinClass" to mixinName,
                        "configPath" to request.mixinConfigPath.orEmpty(),
                    ),
                ),
            )
        }
        val duplicates = listed.groupingBy { it }.eachCount().filter { it.value > 1 }.keys
        return duplicates.map { dup ->
            McDiagnostic(
                code = MixinDiagnosticCodes.DUPLICATE_MIXIN_CONFIG_ENTRY,
                severity = McSeverity.WARNING,
                message = "Duplicate mixin config entry: $dup",
                range = configEntryRange(configContent, dup),
                metadata = mapOf("mixinClass" to dup),
            )
        }
    }

    private fun configEntryRange(configContent: String, mixinClassName: String): McTextRange {
        val quoted = "\"$mixinClassName\""
        val start = configContent.indexOf(quoted)
        return if (start >= 0) {
            offsetRange(configContent, start + 1, start + quoted.length - 1)
        } else {
            McTextRange(McTextPosition(0, 0), McTextPosition(0, 0))
        }
    }

    private fun analyzeInjectMethods(request: MixinDiagnosticRequest, model: MixinClassModel): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        val mixinTargets = findMixinTargetsFromSource(request.source)
        model.injectors.forEach { injector ->
            injector.methodSelectors.forEach { selector ->
                val methodValue = selector.value
                val range = selector.range
                val name = selector.name
                val descriptor = selector.descriptor
                if (mixinTargets.isEmpty()) return@forEach
                val allMatches = mutableListOf<MethodIndexEntry>()
                for (owner in mixinTargets) {
                    allMatches += classIndex.getMethods(owner).filter { it.name == name }
                }
                when {
                    allMatches.isEmpty() -> {
                        results += McDiagnostic(
                            code = MixinDiagnosticCodes.UNRESOLVED_INJECT_METHOD,
                            severity = McSeverity.ERROR,
                            message = "Unresolved injector method: $methodValue",
                            range = range,
                            metadata = mapOf("method" to methodValue),
                        )
                    }
                    descriptor != null -> {
                        val found = allMatches.any { it.descriptor == descriptor }
                        if (!found) {
                            results += McDiagnostic(
                                code = MixinDiagnosticCodes.DESCRIPTOR_MISMATCH,
                                severity = McSeverity.ERROR,
                                message = "Injector method descriptor mismatch: $methodValue",
                                range = range,
                            )
                        }
                    }
                    allMatches.size > 1 -> {
                        results += McDiagnostic(
                            code = MixinDiagnosticCodes.AMBIGUOUS_INJECT_METHOD,
                            severity = McSeverity.WARNING,
                            message = "Ambiguous injector method '$name'; add descriptor",
                            range = range,
                            metadata = mapOf("method" to name, "count" to allMatches.size.toString()),
                        )
                    }
                }
            }
        }
        return results
    }

    private fun analyzeAtTargets(request: MixinDiagnosticRequest, model: MixinClassModel): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        val mixinTargets = findMixinTargetsFromSource(request.source)
        model.injectors.forEach { injector ->
            injector.atSelectors.forEach { at ->
                val value = at.value
                val owner = mixinTargets.firstOrNull()
                val injectMethods = injector.methodSelectors.ifEmpty { listOf(null) }
                for (injectMethodSelector in injectMethods) {
                    val injectMethod = injectMethodSelector?.name
                    val injectMethodDescriptor = injectMethodSelector?.descriptor
                    if (at.ordinal != null && value == "RETURN" && owner != null && injectMethod != null) {
                        val count = bytecodeIndex.getReturnOrdinalCount(owner, injectMethod, injectMethodDescriptor)
                        if (at.ordinal >= count) {
                            results += McDiagnostic(
                                code = MixinDiagnosticCodes.ORDINAL_OUT_OF_RANGE,
                                severity = McSeverity.ERROR,
                                message = "Ordinal ${at.ordinal} out of range for RETURN (max ${count - 1})",
                                range = at.ordinalRange ?: at.range,
                            )
                        }
                    }
                }
                val targetValue = at.target ?: return@forEach
                val range = at.targetRange ?: at.range
                if (targetValue.isEmpty()) return@forEach
                val parsed = MemberTargetParser.parse(targetValue)
                if (parsed is io.github.mcdev.core.descriptor.DescriptorParseResult.Failure) {
                    results += McDiagnostic(
                        code = MixinDiagnosticCodes.INVALID_AT_TARGET_DESCRIPTOR,
                        severity = McSeverity.ERROR,
                        message = "Invalid @At target descriptor: $targetValue",
                        range = range,
                    )
                    return@forEach
                }
                for (injectMethodSelector in injectMethods) {
                    val injectMethod = injectMethodSelector?.name
                    val injectMethodDescriptor = injectMethodSelector?.descriptor
                    if (owner == null || injectMethod == null) continue
                    val candidates = bytecodeIndex.getAtTargetCandidates(owner, injectMethod, injectMethodDescriptor, value)
                    if (candidates.none { atTargetFormatter.formatTarget(it) == targetValue }) {
                        results += McDiagnostic(
                            code = MixinDiagnosticCodes.UNRESOLVED_AT_TARGET,
                            severity = McSeverity.ERROR,
                            message = "Unresolved @At target: $targetValue",
                            range = range,
                            metadata = mapOf("target" to targetValue, "atValue" to value),
                        )
                    }
                }
            }
        }
        return results
    }

    private fun analyzeOverwriteMethods(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val mixinTargets = findMixinTargetsFromSource(request.source)
        if (mixinTargets.isEmpty()) return emptyList()
        return MixinMemberDeclarationParser.parseOverwriteDeclarations(request.source)
            .flatMap { declaration -> overwriteValidation.validate(mixinTargets, declaration) }
    }

    private fun findMixinTargetsFromSource(source: String): List<String> =
        MixinTargetResolver.resolveTargetsFromSource(source, classIndex)

    private data class InjectorAnnotationSpan(
        val start: Int,
        val end: Int,
        val bodyStart: Int,
        val body: String,
        val methodName: String?,
        val methodDescriptor: String?,
    )

    private data class AtAnnotationSpan(
        val atOffset: Int,
        val bodyStart: Int,
        val bodyEnd: Int,
        val body: String,
    )

    private fun findInjectorAnnotationSpans(source: String): List<InjectorAnnotationSpan> {
        val results = mutableListOf<InjectorAnnotationSpan>()
        AnnotationContextExtractor.findInjectorAnnotationOffsets(source).forEach { atOffset ->
            val parenStart = source.indexOf('(', atOffset)
            if (parenStart < 0) return@forEach
            val close = findMatchingParen(source, parenStart) ?: return@forEach
            val bodyStart = parenStart + 1
            val body = source.substring(bodyStart, close)
            val methodValue = INJECT_METHOD_PATTERN.find(body)?.groupValues?.get(1)
            val (methodName, methodDescriptor) = methodValue?.let(::parseInjectMethodValue) ?: (null to null)
            results += InjectorAnnotationSpan(
                start = atOffset,
                end = close + 1,
                bodyStart = bodyStart,
                body = body,
                methodName = methodName,
                methodDescriptor = methodDescriptor,
            )
        }
        return results
    }

    private fun findAtAnnotationSpans(source: String): List<AtAnnotationSpan> {
        val results = mutableListOf<AtAnnotationSpan>()
        AnnotationContextExtractor.findAnnotationOffsets(source, MixinAnnotation.AT).forEach { at ->
            val paren = source.indexOf('(', at)
            if (paren < 0) return@forEach
            val close = findMatchingParen(source, paren) ?: return@forEach
            results += AtAnnotationSpan(
                atOffset = at,
                bodyStart = paren + 1,
                bodyEnd = close,
                body = source.substring(paren + 1, close),
            )
        }
        return results
    }

    private fun findContainingInjector(
        atOffset: Int,
        injectors: List<InjectorAnnotationSpan>,
    ): InjectorAnnotationSpan? =
        injectors
            .filter { atOffset in it.start until it.end }
            .maxByOrNull { it.start }

    private fun parseInjectMethodValue(methodValue: String): Pair<String?, String?> {
        val parenIndex = methodValue.indexOf('(')
        if (parenIndex < 0) return methodValue to null
        return methodValue.substring(0, parenIndex) to methodValue.substring(parenIndex)
    }

    private fun quotedValueRange(
        source: String,
        body: String,
        bodyStart: Int,
        match: MatchResult,
    ): McTextRange {
        val openQuote = bodyStart + match.range.first + match.value.indexOf('"')
        val closeQuote = bodyStart + match.range.last
        return offsetRange(source, openQuote + 1, closeQuote)
    }

    private fun findMatchingParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') return null
        var depth = 0
        var inString = false
        var i = openIndex
        while (i < source.length) {
            when {
                inString -> {
                    if (source[i] == '\\') {
                        i += 2
                        continue
                    }
                    if (source[i] == '"') inString = false
                }
                source[i] == '"' -> inString = true
                source[i] == '(' -> depth++
                source[i] == ')' -> {
                    depth--
                    if (depth == 0) return i
                }
            }
            i++
        }
        return null
    }

    private fun offsetRange(source: String, start: Int, end: Int): McTextRange {
        val startPos = offsetToPosition(source, start)
        val endPos = offsetToPosition(source, end)
        return McTextRange(startPos, endPos)
    }

    private fun offsetToPosition(source: String, offset: Int): McTextPosition {
        var line = 0
        var character = 0
        var i = 0
        while (i < offset && i < source.length) {
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

    private companion object {
        val INJECT_METHOD_PATTERN = Regex("""method\s*=\s*"([^"]+)"""")
        val AT_VALUE_PATTERN = Regex("""value\s*=\s*"([^"]+)"""")
        val AT_SHORT_VALUE_PATTERN = Regex(""""([^"]+)"""")
        val AT_ORDINAL_PATTERN = Regex("""ordinal\s*=\s*(\d+)""")
        val AT_TARGET_PATTERN = Regex("""target\s*=\s*"([^"]+)"""")
    }
}
