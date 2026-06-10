package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace

object SrgParser {
    fun parse(text: String): MappingParseResult {
        val classes = mutableListOf<ClassMapping>()
        val methods = mutableListOf<MethodMapping>()
        val fields = mutableListOf<FieldMapping>()
        val classByObfuscated = mutableMapOf<String, ClassMapping>()

        text.lineSequence().forEachIndexed { index, rawLine ->
            val line = rawLine.trim()
            if (line.isEmpty() || line.startsWith("#")) return@forEachIndexed
            val parts = line.split(Regex("\\s+"))
            when (parts.firstOrNull()) {
                "CL:" -> {
                    if (parts.size != 3) return MappingParseResult.Failure(index + 1, "invalid CL row")
                    val mapping = ClassMapping(
                        mapOf(
                            MappingNamespace.OFFICIAL to parts[1],
                            MappingNamespace.SRG to parts[2],
                        ),
                    )
                    classes += mapping
                    classByObfuscated[parts[1]] = mapping
                }
                "FD:" -> {
                    if (parts.size != 3) return MappingParseResult.Failure(index + 1, "invalid FD row")
                    val from = splitOwnerAndName(parts[1]) ?: return MappingParseResult.Failure(index + 1, "invalid source field")
                    val to = splitOwnerAndName(parts[2]) ?: return MappingParseResult.Failure(index + 1, "invalid target field")
                    val owner = classByObfuscated[from.first] ?: ClassMapping(mapOf(MappingNamespace.OFFICIAL to from.first))
                    fields += FieldMapping(
                        owner,
                        descriptor = "",
                        names = mapOf(MappingNamespace.OFFICIAL to from.second, MappingNamespace.SRG to to.second),
                    )
                }
                "MD:" -> {
                    if (parts.size != 5) return MappingParseResult.Failure(index + 1, "invalid MD row")
                    val from = splitOwnerAndName(parts[1]) ?: return MappingParseResult.Failure(index + 1, "invalid source method")
                    val to = splitOwnerAndName(parts[3]) ?: return MappingParseResult.Failure(index + 1, "invalid target method")
                    val owner = classByObfuscated[from.first] ?: ClassMapping(mapOf(MappingNamespace.OFFICIAL to from.first))
                    methods += MethodMapping(
                        owner,
                        descriptor = parts[2],
                        names = mapOf(MappingNamespace.OFFICIAL to from.second, MappingNamespace.SRG to to.second),
                    )
                }
                else -> return MappingParseResult.Failure(index + 1, "unsupported SRG row")
            }
        }

        return MappingParseResult.Success(
            MappingSet(
                namespaces = listOf(MappingNamespace.OFFICIAL, MappingNamespace.SRG),
                classes = classes,
                fields = fields,
                methods = methods,
            ),
        )
    }

    private fun splitOwnerAndName(value: String): Pair<String, String>? {
        val index = value.lastIndexOf('/')
        if (index <= 0 || index == value.lastIndex) return null
        return value.substring(0, index) to value.substring(index + 1)
    }
}
