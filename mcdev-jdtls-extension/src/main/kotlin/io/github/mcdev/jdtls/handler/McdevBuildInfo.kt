package io.github.mcdev.jdtls.handler

import java.util.Properties

data class McdevBuildIdentity(
    val commit: String = "unknown",
    val buildTime: String = "unknown",
    val version: String = "unknown",
    val jarLocation: String = "unknown",
)

object McdevBuildInfo {
    fun load(): McdevBuildIdentity {
        val properties = Properties()
        runCatching {
            McdevBuildInfo::class.java.classLoader
                .getResourceAsStream("mcdev-build.properties")
                ?.use(properties::load)
        }
        return McdevBuildIdentity(
            commit = properties.getProperty("commit", "unknown"),
            buildTime = properties.getProperty("buildTime", "unknown"),
            version = properties.getProperty("version", "unknown"),
            jarLocation = runCatching {
                McdevBuildInfo::class.java.protectionDomain.codeSource.location.toString()
            }.getOrDefault("unknown"),
        )
    }
}
