package io.github.mcdev.core.at

import io.github.mcdev.core.descriptor.DescriptorParseResult
import io.github.mcdev.core.descriptor.parseMethodDescriptor
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.diagnostics.McSeverity
import io.github.mcdev.core.mapping.ProjectMappingContext
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.model.MemberKind

@Deprecated("Use AtDiagnosticRequest", ReplaceWith("AtDiagnosticRequest"))
typealias AccessTransformerDiagnosticRequest = AtDiagnosticRequest

class AccessTransformerDiagnosticsService(
    private val classIndex: ClassIndex,
    private val mappingContext: ProjectMappingContext? = null,
    private val memberResolver: AtMemberResolver = AtMemberResolver(),
    private val editor: AccessTransformerEditor = AccessTransformerEditor(),
) {
    fun analyze(request: AtDiagnosticRequest): List<McDiagnostic> {
        val index = request.classIndex ?: classIndex
        val mappings = request.mappingContext ?: mappingContext
        val parseResult = AccessTransformerParser.parse(request.source)
        val diagnostics = mutableListOf<McDiagnostic>()
        when (parseResult) {
            is AccessTransformerParseResult.Failure -> {
                diagnostics += parseFailureDiagnostic(request.source, parseResult)
            }
            is AccessTransformerParseResult.Success -> {
                diagnostics += analyzeEntries(request.source, parseResult.file.entries, index, mappings)
            }
        }
        return diagnostics
    }

    private fun parseFailureDiagnostic(
        source: String,
        failure: AccessTransformerParseResult.Failure,
    ): McDiagnostic {
        val range = AtTextPositions.lineRange(source, failure.line)
            ?: AtTextPositions.rangeForOffsets(source, 0, source.length)
        val code = when {
            failure.message.contains("modifier") -> AtDiagnosticCodes.INVALID_MODIFIER
            failure.message.contains("descriptor") -> AtDiagnosticCodes.INVALID_DESCRIPTOR
            else -> AtDiagnosticCodes.PARSE_ERROR
        }
        return McDiagnostic(
            code = code,
            severity = McSeverity.ERROR,
            message = failure.message,
            range = range,
            metadata = mapOf("line" to failure.line.toString()),
        )
    }

    private fun analyzeEntries(
        source: String,
        entries: List<AccessTransformerEntry>,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): List<McDiagnostic> {
        val diagnostics = mutableListOf<McDiagnostic>()
        val seen = linkedMapOf<String, AccessTransformerEntry>()
        for (entry in entries) {
            val range = entryRange(source, entry) ?: continue
            diagnostics += analyzeEntry(entry, range, classIndex, mappingContext)
            val key = editor.entryKey(entry)
            val previous = seen.put(key, entry)
            if (previous != null) {
                diagnostics += McDiagnostic(
                    code = AtDiagnosticCodes.DUPLICATE_ENTRY,
                    severity = McSeverity.WARNING,
                    message = "Duplicate access transformer entry",
                    range = range,
                    metadata = mapOf(
                        "line" to entry.line.toString(),
                        "duplicateOfLine" to previous.line.toString(),
                    ),
                )
            }
        }
        return diagnostics
    }

    private fun analyzeEntry(
        entry: AccessTransformerEntry,
        range: io.github.mcdev.core.diagnostics.McTextRange,
        classIndex: ClassIndex,
        mappingContext: ProjectMappingContext?,
    ): List<McDiagnostic> {
        val ownerEntry = classIndex.findClassByFqn(entry.owner)
            ?: classIndex.findClass(entry.owner.replace('.', '/'))
        if (ownerEntry == null) {
            return listOf(
                McDiagnostic(
                    code = AtDiagnosticCodes.UNRESOLVED_CLASS,
                    severity = McSeverity.ERROR,
                    message = "Unresolved class '${entry.owner}'",
                    range = range,
                    metadata = mapOf("owner" to entry.owner, "line" to entry.line.toString()),
                ),
            )
        }

        val memberName = entry.name ?: return emptyList()
        when (val resolution = memberResolver.resolve(
            ownerInternalName = ownerEntry.internalName,
            memberName = memberName,
            memberDescriptor = entry.descriptor,
            classIndex = classIndex,
            mappingContext = mappingContext,
        )) {
            is AtMemberResolution.NotFound -> {
                return listOf(
                    McDiagnostic(
                        code = AtDiagnosticCodes.UNRESOLVED_MEMBER,
                        severity = McSeverity.ERROR,
                        message = "Unresolved member '$memberName' in '${entry.owner}'",
                        range = range,
                        metadata = mapOf(
                            "owner" to entry.owner,
                            "member" to memberName,
                            "line" to entry.line.toString(),
                        ),
                    ),
                )
            }
            is AtMemberResolution.WrongNamespace -> {
                return listOf(
                    McDiagnostic(
                        code = AtDiagnosticCodes.WRONG_NAMESPACE,
                        severity = McSeverity.ERROR,
                        message = "Member '${resolution.actualName}' should use ${resolution.expectedNamespace.name.lowercase()} names",
                        range = range,
                        metadata = mapOf(
                            "owner" to entry.owner,
                            "member" to memberName,
                            "expectedNamespace" to resolution.expectedNamespace.name,
                            "line" to entry.line.toString(),
                        ),
                    ),
                )
            }
            is AtMemberResolution.MappingNotFound -> {
                return listOf(
                    McDiagnostic(
                        code = AtDiagnosticCodes.SRG_MAPPING_NOT_FOUND,
                        severity = McSeverity.ERROR,
                        message = "SRG mapping not found for ${resolution.kind.name.lowercase()} '${resolution.namedName}'",
                        range = range,
                        metadata = mapOf(
                            "owner" to entry.owner,
                            "member" to resolution.namedName,
                            "line" to entry.line.toString(),
                        ),
                    ),
                )
            }
            is AtMemberResolution.Found -> {
                return validateFoundMember(entry, resolution, range)
            }
            is AtMemberResolution.DescriptorMismatch -> {
                val expectedDescriptors = resolution.candidates.joinToString(", ") { it.descriptor }
                return listOf(
                    McDiagnostic(
                        code = AtDiagnosticCodes.INVALID_DESCRIPTOR,
                        severity = McSeverity.ERROR,
                        message = "Descriptor '${resolution.descriptor}' does not match any descriptor for '${resolution.namedName}'",
                        range = range,
                        metadata = mapOf(
                            "line" to entry.line.toString(),
                            "member" to resolution.namedName,
                            "descriptor" to resolution.descriptor,
                            "expectedDescriptors" to expectedDescriptors,
                        ),
                    ),
                )
            }
            is AtMemberResolution.MissingDescriptor -> {
                return listOf(
                    McDiagnostic(
                        code = AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR,
                        severity = McSeverity.ERROR,
                        message = "Missing descriptor for method '${resolution.namedName}'",
                        range = range,
                        metadata = mapOf(
                            "owner" to entry.owner,
                            "member" to resolution.namedName,
                            "line" to entry.line.toString(),
                        ),
                    ),
                )
            }
        }
    }

    private fun validateFoundMember(
        entry: AccessTransformerEntry,
        resolution: AtMemberResolution.Found,
        range: io.github.mcdev.core.diagnostics.McTextRange,
    ): List<McDiagnostic> {
        if (resolution.member.kind != MemberKind.METHOD) return emptyList()
        val method = resolution.member.method ?: return emptyList()
        if (entry.descriptor == null) {
            return listOf(
                McDiagnostic(
                    code = AtDiagnosticCodes.MISSING_METHOD_DESCRIPTOR,
                    severity = McSeverity.ERROR,
                    message = "Missing descriptor for method '${resolution.member.namedName}'",
                    range = range,
                    metadata = mapOf(
                        "owner" to entry.owner,
                        "member" to resolution.member.namedName,
                        "descriptor" to method.descriptor,
                        "line" to entry.line.toString(),
                    ),
                ),
            )
        }
        if (parseMethodDescriptor(entry.descriptor) is DescriptorParseResult.Failure) {
            return listOf(
                McDiagnostic(
                    code = AtDiagnosticCodes.INVALID_DESCRIPTOR,
                    severity = McSeverity.ERROR,
                    message = "Invalid method descriptor '${entry.descriptor}'",
                    range = range,
                    metadata = mapOf("line" to entry.line.toString()),
                ),
            )
        }
        if (entry.descriptor != method.descriptor && resolution.member.descriptor != method.descriptor) {
            return listOf(
                McDiagnostic(
                    code = AtDiagnosticCodes.INVALID_DESCRIPTOR,
                    severity = McSeverity.ERROR,
                    message = "Descriptor '${entry.descriptor}' does not match '${method.descriptor}'",
                    range = range,
                    metadata = mapOf("line" to entry.line.toString()),
                ),
            )
        }
        return emptyList()
    }

    private fun entryRange(source: String, entry: AccessTransformerEntry): io.github.mcdev.core.diagnostics.McTextRange? =
        AtTextPositions.lineRange(source, entry.line)
}
