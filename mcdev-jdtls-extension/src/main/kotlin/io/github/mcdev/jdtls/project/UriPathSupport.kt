package io.github.mcdev.jdtls.project

import java.nio.file.Path
import java.nio.file.Paths

internal object UriPathSupport {
    fun uriToPath(uri: String): Path {
        val trimmed = uri.trim()
        if (trimmed.isEmpty()) {
            error("workspace URI is empty")
        }
        val withoutScheme = trimmed.removePrefix("file://")
        val normalized = if (withoutScheme.startsWith("/") && WINDOWS_DRIVE_URI.matches(withoutScheme)) {
            withoutScheme.drop(1)
        } else {
            withoutScheme
        }
        return Paths.get(normalized)
    }

    fun pathToUri(path: Path): String = path.toUri().toString()

    private val WINDOWS_DRIVE_URI = Regex("^/[A-Za-z]:/.*")
}
