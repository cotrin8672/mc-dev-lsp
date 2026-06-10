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
) {
    fun analyze(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        diagnostics += analyzeMixinTargets(request)
        diagnostics += analyzeMixinConfig(request)
        diagnostics += analyzeInjectMethods(request)
        diagnostics += analyzeAtTargets(request)
        return diagnostics
    }

    private fun analyzeMixinTargets(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        val pattern = Regex("""@Mixin\s*\(([^)]*)\)""")
        pattern.findAll(request.source).forEach { match ->
            val body = match.groupValues[1]
            val range = offsetRange(request.source, match.range.first, match.range.last + 1)
            val targets = AnnotationContextExtractor.parseMixinTargetValues(request.source, match.range.first)
            results += duplicateMixinTargetDiagnostics(request.source, match.range.first, range)
            if (targets.isEmpty() && !body.contains(".class")) return@forEach
            for (target in targets) {
                if (MixinTargetResolver.resolveTarget(target, classIndex) == null) {
                    results += McDiagnostic(
                        code = MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET,
                        severity = McSeverity.ERROR,
                        message = "Unresolved @Mixin target: $target",
                        range = range,
                        metadata = mapOf("target" to target),
                    )
                }
            }
            val classRefs = Regex("""([\w.]+)\s*\.class""").findAll(body)
            for (ref in classRefs) {
                val fqn = ref.groupValues[1]
                if (MixinTargetResolver.resolveTarget(fqn, classIndex) == null) {
                    results += McDiagnostic(
                        code = MixinDiagnosticCodes.UNRESOLVED_MIXIN_TARGET,
                        severity = McSeverity.ERROR,
                        message = "Unresolved @Mixin target: $fqn",
                        range = range,
                        metadata = mapOf("target" to fqn),
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
                range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
                metadata = mapOf("mixinClass" to dup),
            )
        }
    }

    private fun analyzeInjectMethods(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        val pattern = Regex("""@(Inject|Redirect|ModifyArg|ModifyArgs|ModifyVariable|ModifyConstant)\s*\(([^)]*)\)""")
        val mixinTargets = findMixinTargetsFromSource(request.source)
        pattern.findAll(request.source).forEach { match ->
            val methodMatch = Regex("""method\s*=\s*"([^"]+)""").find(match.groupValues[2]) ?: return@forEach
            val methodValue = methodMatch.groupValues[1]
            val range = offsetRange(request.source, methodMatch.range.first, methodMatch.range.last + 1)
            val name = methodValue.substringBefore('(')
            val descriptorSuffix = methodValue.substringAfter('(', "")
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
                descriptorSuffix.isNotEmpty() -> {
                    val fullDescriptor = "($descriptorSuffix"
                    val found = allMatches.any { it.descriptor == fullDescriptor || methodValue.endsWith(it.descriptor) }
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
        return results
    }

    private fun analyzeAtTargets(request: MixinDiagnosticRequest): List<McDiagnostic> {
        val results = mutableListOf<McDiagnostic>()
        val mixinTargets = findMixinTargetsFromSource(request.source)
        val injectMethod = findFirstInjectMethod(request.source)
        AnnotationContextExtractor.extractAtAnnotationBodies(request.source).forEach { body ->
            val value = Regex("""value\s*=\s*"([^"]+)""").find(body)?.groupValues?.get(1) ?: return@forEach
            val owner = mixinTargets.firstOrNull()
            val ordinalMatch = Regex("""ordinal\s*=\s*(\d+)""").find(body)
            if (ordinalMatch != null && value == "RETURN" && owner != null) {
                val ordinal = ordinalMatch.groupValues[1].toIntOrNull()
                if (ordinal != null) {
                    val count = bytecodeIndex.getReturnOrdinalCount(owner, injectMethod ?: "", null)
                    if (ordinal >= count) {
                        results += McDiagnostic(
                            code = MixinDiagnosticCodes.ORDINAL_OUT_OF_RANGE,
                            severity = McSeverity.ERROR,
                            message = "Ordinal $ordinal out of range for RETURN (max ${count - 1})",
                            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
                        )
                    }
                }
            }
            val targetMatch = Regex("""target\s*=\s*"([^"]+)""").find(body) ?: return@forEach
            val targetValue = targetMatch.groupValues[1]
            val range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 0))
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
            if (owner == null) return@forEach
            val candidates = bytecodeIndex.getAtTargetCandidates(owner, injectMethod ?: "", null, value)
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
        return results
    }

    private fun findMixinTargetsFromSource(source: String): List<String> =
        MixinTargetResolver.resolveTargetsFromSource(source, classIndex)

    private fun findFirstInjectMethod(source: String): String? =
        Regex("""method\s*=\s*"([^"(]+)""").find(source)?.groupValues?.get(1)

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
}
