package io.github.mcdev.core.project

import java.nio.file.Path

data class MixinConfigRef(
    val path: Path,
    val packageName: String? = null,
    val mixins: List<String> = emptyList(),
    val client: List<String> = emptyList(),
    val server: List<String> = emptyList(),
    val common: List<String> = emptyList(),
)
