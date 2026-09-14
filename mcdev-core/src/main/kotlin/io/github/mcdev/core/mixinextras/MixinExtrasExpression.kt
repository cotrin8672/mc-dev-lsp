package io.github.mcdev.core.mixinextras

data class MixinExtrasExpression(
    val id: String = "",
    val values: List<String> = emptyList(),
)

data class MixinExtrasExpressionIndex(
    val expressions: List<MixinExtrasExpression> = emptyList(),
) {
    fun valuesForId(id: String): List<String> =
        expressions
            .filter { it.id == id }
            .flatMap { it.values }
}
