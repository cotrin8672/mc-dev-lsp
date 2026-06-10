package io.github.mcdev.core.project

import java.nio.file.Path

internal object ProjectPathFilters {
    private val EXCLUDED_DIRECTORY_NAMES = setOf("build", ".gradle", "node_modules")

    fun isUnderExcludedDirectory(path: Path): Boolean {
        for (index in 0 until path.nameCount) {
            if (path.getName(index).toString().lowercase() in EXCLUDED_DIRECTORY_NAMES) {
                return true
            }
        }
        return false
    }
}
