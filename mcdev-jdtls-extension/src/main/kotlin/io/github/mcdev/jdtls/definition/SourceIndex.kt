package io.github.mcdev.jdtls.definition

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.project.UriPathSupport
import io.github.mcdev.protocol.McdevDefinitionResolution
import kotlin.io.path.extension

class SourceIndex private constructor(
    private val files: List<IndexedJavaFile>,
) {
    fun resolve(target: McDefinitionTarget): ResolvedDefinition {
        val ownerFqn = target.ownerFqn
            ?: internalNameToFqn(target.ownerInternalName)
        val file = files.find { it.fqn == ownerFqn }
            ?: return unresolved(
                target = target,
                message = "no project source file for $ownerFqn",
            )

        return when (target.kind) {
            MemberKind.CLASS -> resolveClass(file, target)
            MemberKind.FIELD -> resolveField(file, target)
            MemberKind.METHOD -> resolveMethod(file, target)
            else -> unresolved(target, "unsupported member kind ${target.kind}")
        }
    }

    private fun resolveClass(file: IndexedJavaFile, target: McDefinitionTarget): ResolvedDefinition {
        val match = CLASS_PATTERN.find(file.text) ?: return unresolved(target, "class declaration not found in ${file.fqn}")
        return resolved(target, file.documentUri, file.rangeFor(match.range))
    }

    private fun resolveField(file: IndexedJavaFile, target: McDefinitionTarget): ResolvedDefinition {
        val fieldName = target.name ?: return unresolved(target, "field name missing")
        val pattern = fieldPattern(fieldName)
        val match = pattern.findAll(file.text).firstOrNull { candidate ->
            !isInsideComment(file.text, candidate.range.first)
        } ?: return unresolved(target, "field '$fieldName' not found in ${file.fqn}")
        return resolved(target, file.documentUri, file.rangeFor(match.range))
    }

    private fun resolveMethod(file: IndexedJavaFile, target: McDefinitionTarget): ResolvedDefinition {
        val methodName = target.name ?: return unresolved(target, "method name missing")
        val descriptor = target.descriptor
        val candidates = METHOD_PATTERN.findAll(file.text)
            .filter { it.groupValues[1] == methodName && !isInsideComment(file.text, it.range.first) }
            .toList()
        if (candidates.isEmpty()) {
            return unresolved(target, "method '$methodName' not found in ${file.fqn}")
        }
        val match = when {
            descriptor.isNullOrBlank() -> candidates.singleOrNull() ?: candidates.first()
            else -> {
                candidates.firstOrNull { candidate ->
                    sourceParametersToDescriptor(file, candidate.groupValues[2]) == descriptor.substringBefore(')') + ")"
                } ?: candidates.singleOrNull() ?: candidates.first()
            }
        }
        val nameStart = match.range.first + match.value.indexOf(methodName)
        val nameEnd = nameStart + methodName.length
        return resolved(target, file.documentUri, file.rangeForOffsets(nameStart, nameEnd))
    }

    private fun resolved(
        target: McDefinitionTarget,
        documentUri: String,
        range: McTextRange,
    ): ResolvedDefinition =
        ResolvedDefinition(
            target = target,
            documentUri = documentUri,
            range = range,
            resolution = McdevDefinitionResolution.SOURCE,
        )

    private fun unresolved(target: McDefinitionTarget, message: String): ResolvedDefinition =
        ResolvedDefinition(
            target = target,
            documentUri = "",
            range = target.sourceRange ?: McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
            resolution = McdevDefinitionResolution.UNRESOLVED,
            resolutionMessage = message,
        )

    companion object {
        private val PACKAGE_PATTERN = Regex("""^\s*package\s+([\w.]+)\s*;""", RegexOption.MULTILINE)
        private val CLASS_PATTERN = Regex(
            """(?:public\s+|protected\s+|private\s+)?(?:abstract\s+|static\s+)?(?:class|interface|enum)\s+(\w+)""",
        )
        private val METHOD_PATTERN = Regex(
            """(?m)(?:public|protected|private)?\s*(?:static\s+)?(?:abstract\s+)?[\w.<>\[\]]+\s+(\w+)\s*\(([^)]*)\)""",
        )

        fun fromProject(projectContext: ProjectContext): SourceIndex {
            val entries = mutableListOf<SourceScanEntry>()
            projectContext.sourceSets.forEach { sourceSet ->
                sourceSet.sourceDirectories.forEach { sourceDir ->
                    if (!sourceDir.toFile().isDirectory) return@forEach
                    sourceDir.toFile().walkTopDown()
                        .filter { it.isFile && it.extension == "java" }
                        .forEach { file ->
                            entries += SourceScanEntry(
                                documentUri = UriPathSupport.pathToUri(file.toPath()),
                                text = runCatching { file.readText() }.getOrDefault(""),
                            )
                        }
                }
            }
            return fromEntries(entries)
        }

        fun fromEntries(entries: List<SourceScanEntry>): SourceIndex {
            val files = entries.mapNotNull { entry ->
                if (!entry.documentUri.endsWith(".java", ignoreCase = true)) return@mapNotNull null
                val packageName = PACKAGE_PATTERN.find(entry.text)?.groupValues?.get(1).orEmpty()
                val imports = IMPORT_PATTERN.findAll(entry.text)
                    .associate { match ->
                        val fqn = match.groupValues[1]
                        fqn.substringAfterLast('.') to fqn
                    }
                val className = CLASS_PATTERN.find(entry.text)?.groupValues?.get(1) ?: return@mapNotNull null
                val fqn = if (packageName.isEmpty()) className else "$packageName.$className"
                IndexedJavaFile(
                    documentUri = entry.documentUri,
                    text = entry.text,
                    fqn = fqn,
                    packageName = packageName,
                    imports = imports,
                )
            }
            return SourceIndex(files)
        }

        private fun internalNameToFqn(internalName: String): String =
            internalName.replace('/', '.')

        private fun fieldPattern(fieldName: String): Regex =
            Regex(
                """(?m)(?:public|protected|private)?\s*(?:static\s+)?(?:final\s+)?[\w.<>\[\]]+\s+""" +
                    Regex.escape(fieldName) + """\s*[;=]""",
            )

        private val IMPORT_PATTERN = Regex("""^\s*import\s+([\w.]+)\s*;""", RegexOption.MULTILINE)
        private val PARAMETER_MODIFIER_PATTERN = Regex("""\b(?:final|volatile)\s+""")
        private val ANNOTATION_PATTERN = Regex("""@\w+(?:\([^)]*\))?\s*""")
        private val JAVA_LANG_TYPES = setOf(
            "Boolean",
            "Byte",
            "Character",
            "Class",
            "Double",
            "Float",
            "Integer",
            "Long",
            "Object",
            "Short",
            "String",
            "Void",
        )

        private fun sourceParametersToDescriptor(file: IndexedJavaFile, parameterList: String): String {
            val trimmed = parameterList.trim()
            if (trimmed.isEmpty()) return "()"
            return trimmed.split(',')
                .joinToString(prefix = "(", postfix = ")") { parameter ->
                    sourceTypeToDescriptor(file, parameterToType(parameter))
                }
        }

        private fun parameterToType(parameter: String): String {
            val cleaned = parameter
                .replace(ANNOTATION_PATTERN, "")
                .replace(PARAMETER_MODIFIER_PATTERN, "")
                .trim()
            val beforeName = cleaned.substringBeforeLast(' ', cleaned)
            return beforeName.replace("...", "[]").trim()
        }

        private fun sourceTypeToDescriptor(file: IndexedJavaFile, rawType: String): String {
            var type = rawType.substringBefore('<').trim()
            var arrays = 0
            while (type.endsWith("[]")) {
                arrays++
                type = type.removeSuffix("[]").trim()
            }
            val base = when (type) {
                "void" -> "V"
                "boolean" -> "Z"
                "byte" -> "B"
                "char" -> "C"
                "short" -> "S"
                "int" -> "I"
                "long" -> "J"
                "float" -> "F"
                "double" -> "D"
                else -> objectTypeDescriptor(file, type)
            }
            return "[".repeat(arrays) + base
        }

        private fun objectTypeDescriptor(file: IndexedJavaFile, type: String): String {
            val fqn = when {
                type.contains('.') -> type
                file.imports.containsKey(type) -> file.imports.getValue(type)
                type in JAVA_LANG_TYPES -> "java.lang.$type"
                file.packageName.isNotEmpty() -> "${file.packageName}.$type"
                else -> type
            }
            return "L${fqn.replace('.', '/')};"
        }

        private fun isInsideComment(text: String, offset: Int): Boolean {
            val lineStart = text.lastIndexOf('\n', offset.coerceAtLeast(0) - 1) + 1
            val lineEnd = text.indexOf('\n', offset).let { if (it < 0) text.length else it }
            val line = text.substring(lineStart, lineEnd)
            return line.trimStart().startsWith("//")
        }
    }
}

private data class IndexedJavaFile(
    val documentUri: String,
    val text: String,
    val fqn: String,
    val packageName: String,
    val imports: Map<String, String>,
) {
    fun rangeFor(range: IntRange): McTextRange {
        val start = offsetToPosition(range.first)
        val end = offsetToPosition(range.last + 1)
        return McTextRange(start, end)
    }

    fun rangeForOffsets(startOffset: Int, endOffsetExclusive: Int): McTextRange {
        val start = offsetToPosition(startOffset)
        val end = offsetToPosition(endOffsetExclusive)
        return McTextRange(start, end)
    }

    private fun offsetToPosition(offset: Int): McTextPosition {
        val safeOffset = offset.coerceIn(0, text.length)
        val before = text.substring(0, safeOffset)
        val line = before.count { it == '\n' }
        val lineStart = before.lastIndexOf('\n') + 1
        return McTextPosition(line = line, character = safeOffset - lineStart)
    }
}
