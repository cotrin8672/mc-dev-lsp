package io.github.mcdev.core.mapping

import io.github.mcdev.core.model.MappingNamespace

data class ClassMapping(
    val names: Map<MappingNamespace, String>,
)

data class FieldMapping(
    val owner: ClassMapping,
    val descriptor: String,
    val names: Map<MappingNamespace, String>,
)

data class MethodMapping(
    val owner: ClassMapping,
    val descriptor: String,
    val names: Map<MappingNamespace, String>,
)

data class MappingSet(
    val namespaces: List<MappingNamespace>,
    val classes: List<ClassMapping>,
    val fields: List<FieldMapping>,
    val methods: List<MethodMapping>,
) {
    fun className(name: String, from: MappingNamespace, to: MappingNamespace): String? =
        classes.firstOrNull { it.names[from] == name }?.names?.get(to)

    fun methodName(owner: String, name: String, descriptor: String, from: MappingNamespace, to: MappingNamespace): String? =
        methods.firstOrNull {
            it.owner.names[from] == owner && it.names[from] == name && it.descriptor == descriptor
        }?.names?.get(to)

    fun fieldName(owner: String, name: String, descriptor: String, from: MappingNamespace, to: MappingNamespace): String? =
        fields.firstOrNull {
            it.owner.names[from] == owner && it.names[from] == name && it.descriptor == descriptor
        }?.names?.get(to)
}

sealed interface MappingParseResult {
    data class Success(val mappings: MappingSet) : MappingParseResult
    data class Failure(val line: Int, val message: String) : MappingParseResult
}

fun parseNamespace(value: String): MappingNamespace? = when (value.lowercase()) {
    "named" -> MappingNamespace.NAMED
    "intermediary" -> MappingNamespace.INTERMEDIARY
    "official" -> MappingNamespace.OFFICIAL
    "srg" -> MappingNamespace.SRG
    "mcp" -> MappingNamespace.MCP
    else -> null
}
