package io.github.mcdev.core.awat

enum class AwAtFileType {
    ACCESS_WIDENER,
    ACCESS_TRANSFORMER,
}

object AwAtBufferSupport {
    fun detectFileType(languageId: String, documentUri: String): AwAtFileType? {
        when (languageId.lowercase()) {
            "accesswidener" -> return AwAtFileType.ACCESS_WIDENER
            "accesstransformer" -> return AwAtFileType.ACCESS_TRANSFORMER
        }
        val path = documentUri.substringAfterLast('/').lowercase()
        if (path.endsWith(".accesswidener") || path.endsWith(".aw")) {
            return AwAtFileType.ACCESS_WIDENER
        }
        if (path.endsWith("_at.cfg") || path == "accesstransformer.cfg" || path.endsWith(".at")) {
            return AwAtFileType.ACCESS_TRANSFORMER
        }
        return null
    }
}
