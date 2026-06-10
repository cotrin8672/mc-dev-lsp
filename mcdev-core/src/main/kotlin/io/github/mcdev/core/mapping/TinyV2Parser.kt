package io.github.mcdev.core.mapping

object TinyV2Parser {
    fun parse(text: String): MappingParseResult {
        val lines = text.lineSequence().toList()
        if (lines.isEmpty()) return MappingParseResult.Failure(1, "empty Tiny v2 mapping")
        val header = lines.first().split('\t')
        if (header.size < 4 || header[0] != "tiny" || header[1] != "2") {
            return MappingParseResult.Failure(1, "expected Tiny v2 header")
        }
        val namespaces = header.drop(3).mapIndexed { index, value ->
            parseNamespace(value) ?: return MappingParseResult.Failure(1, "unknown namespace '$value' at column ${index + 4}")
        }
        val classes = mutableListOf<ClassMapping>()
        val fields = mutableListOf<FieldMapping>()
        val methods = mutableListOf<MethodMapping>()
        var currentClass: ClassMapping? = null

        lines.drop(1).forEachIndexed { zeroIndex, rawLine ->
            val lineNumber = zeroIndex + 2
            if (rawLine.isBlank() || rawLine.startsWith("#")) return@forEachIndexed
            val indent = rawLine.takeWhile { it == '\t' }.length
            val parts = rawLine.trimStart('\t').split('\t')
            when {
                indent == 0 && parts.firstOrNull() == "c" -> {
                    if (parts.size != namespaces.size + 1) {
                        return MappingParseResult.Failure(lineNumber, "class mapping has wrong number of names")
                    }
                    currentClass = ClassMapping(namespaces.zip(parts.drop(1)).toMap())
                    classes += currentClass!!
                }
                indent == 1 && parts.firstOrNull() == "m" -> {
                    val owner = currentClass ?: return MappingParseResult.Failure(lineNumber, "method mapping without class")
                    if (parts.size != namespaces.size + 2) {
                        return MappingParseResult.Failure(lineNumber, "method mapping has wrong number of names")
                    }
                    methods += MethodMapping(owner, parts[1], namespaces.zip(parts.drop(2)).toMap())
                }
                indent == 1 && parts.firstOrNull() == "f" -> {
                    val owner = currentClass ?: return MappingParseResult.Failure(lineNumber, "field mapping without class")
                    if (parts.size != namespaces.size + 2) {
                        return MappingParseResult.Failure(lineNumber, "field mapping has wrong number of names")
                    }
                    fields += FieldMapping(owner, parts[1], namespaces.zip(parts.drop(2)).toMap())
                }
                else -> return MappingParseResult.Failure(lineNumber, "unsupported Tiny v2 row")
            }
        }

        return MappingParseResult.Success(MappingSet(namespaces, classes, fields, methods))
    }
}
