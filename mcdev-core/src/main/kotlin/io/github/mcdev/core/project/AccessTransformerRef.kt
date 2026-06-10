package io.github.mcdev.core.project

import io.github.mcdev.core.at.AccessTransformerFile
import java.nio.file.Path

data class AccessTransformerRef(
    val path: Path,
    val parsed: AccessTransformerFile? = null,
)
