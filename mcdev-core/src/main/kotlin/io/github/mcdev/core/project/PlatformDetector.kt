package io.github.mcdev.core.project

import java.nio.file.Path
import kotlin.io.path.exists
import kotlin.io.path.isRegularFile
import kotlin.io.path.readText

object PlatformDetector {
    private val GRADLE_FILE_NAMES = listOf(
        "build.gradle",
        "build.gradle.kts",
        "settings.gradle",
        "settings.gradle.kts",
    )

    fun detect(gradleContents: List<String>): ModPlatform {
        val combined = gradleContents.joinToString("\n").lowercase()
        return when {
            combined.contains("fabric-loom") ||
                combined.contains("net.fabricmc.fabric-loom") -> ModPlatform.FABRIC
            combined.contains("net.neoforged.gradle") ||
                combined.contains("org.neoforged.moddev") ||
                (combined.contains("neoforge") && combined.contains("moddev")) -> ModPlatform.NEOFORGE
            combined.contains("forgegradle") ||
                combined.contains("net.minecraftforge.gradle") ||
                combined.contains("minecraftforge") -> ModPlatform.FORGE
            else -> ModPlatform.UNKNOWN
        }
    }

    fun detectFromRoot(root: Path): ModPlatform {
        val contents = GRADLE_FILE_NAMES.mapNotNull { name ->
            val file = root.resolve(name)
            if (file.exists() && file.isRegularFile()) file.readText() else null
        }
        return detect(contents)
    }
}
