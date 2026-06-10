package io.github.mcdev.core.project

import io.github.mcdev.core.aw.AccessWidenerFile
import io.github.mcdev.core.model.MappingNamespace
import java.nio.file.Path

data class AccessWidenerRef(
    val path: Path,
    val namespace: MappingNamespace? = null,
    val parsed: AccessWidenerFile? = null,
)
