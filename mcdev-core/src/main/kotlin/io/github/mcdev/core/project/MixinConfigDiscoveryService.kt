package io.github.mcdev.core.project

import com.google.gson.JsonObject
import com.google.gson.JsonParser
import java.nio.file.Path
import kotlin.io.path.extension
import kotlin.io.path.name
import kotlin.io.path.readText

object MixinConfigDiscoveryService {
    fun discover(root: Path): List<MixinConfigRef> {
        val paths = ProjectTreeWalker.walkRegularFiles(root)
            .filter { isMixinConfigFile(it) }
        return paths.mapNotNull { path -> parse(path) }.sortedBy { it.path.toString() }
    }

    fun isMixinConfigFile(path: Path): Boolean {
        val name = path.name.lowercase()
        val extension = path.extension.lowercase()
        return name == "mixins.json" ||
            name == "mixins.json5" ||
            name.endsWith(".mixins.json") ||
            name.endsWith(".mixins.json5") ||
            (extension == "json" && name.contains("mixin")) ||
            (extension == "json5" && name.contains("mixin"))
    }

    fun parse(path: Path): MixinConfigRef? {
        return try {
            parseContent(path, path.readText())
        } catch (_: Exception) {
            null
        }
    }

    fun parseContent(path: Path, content: String): MixinConfigRef? {
        return try {
            parseJsonContent(path, content)
        } catch (_: Exception) {
            null
        }
    }

    fun normalizeJsonContent(content: String): String = removeTrailingCommas(stripJson5Comments(content))

    fun readContent(path: Path): String? =
        runCatching { path.readText() }.getOrNull()

    fun sharedConfigs(
        configs: List<MixinConfigRef>,
        allSourceSets: List<SourceSetContext>,
        projectRoot: Path,
    ): List<MixinConfigRef> {
        val normalizedRoot = projectRoot.toAbsolutePath().normalize()
        val allResourceDirectories = allSourceSets
            .flatMap { it.resourceDirectories }
            .map { it.toAbsolutePath().normalize() }
            .distinct()

        return configs
            .filter { config ->
                val normalizedPath = config.path.toAbsolutePath().normalize()
                isUnderDirectory(normalizedPath, normalizedRoot) &&
                    allResourceDirectories.none { isUnderDirectory(normalizedPath, it) }
            }
            .sortedBy { it.path.toAbsolutePath().normalize().toString() }
    }

    fun configsForSourceSet(
        configs: List<MixinConfigRef>,
        sourceSet: SourceSetContext,
        allSourceSets: List<SourceSetContext>,
        projectRoot: Path,
    ): List<MixinConfigRef> {
        val normalizedRoot = projectRoot.toAbsolutePath().normalize()
        val selectedResourceDirectories = sourceSet.resourceDirectories
            .map { it.toAbsolutePath().normalize() }

        val inProject = configs.filter { config ->
            isUnderDirectory(config.path.toAbsolutePath().normalize(), normalizedRoot)
        }

        val localConfigs = inProject
            .filter { config ->
                val normalizedPath = config.path.toAbsolutePath().normalize()
                selectedResourceDirectories.any { isUnderDirectory(normalizedPath, it) }
            }
            .sortedBy { it.path.toAbsolutePath().normalize().toString() }
        if (localConfigs.isNotEmpty()) {
            return localConfigs
        }

        return sharedConfigs(configs, allSourceSets, projectRoot)
    }

    fun selectForMixin(
        configs: List<MixinConfigRef>,
        mixinClassName: String?,
        mixinPackage: String?,
    ): MixinConfigRef? {
        if (configs.isEmpty()) return null

        val qualifiedName = buildMixinQualifiedName(mixinPackage, mixinClassName) ?: return null

        if (mixinClassName != null) {
            val listingConfigs = configs.filter { it.listsMixinRelative(qualifiedName, mixinClassName) }
            when (listingConfigs.size) {
                1 -> return listingConfigs.first()
                else -> if (listingConfigs.size > 1) return null
            }
        }

        if (mixinPackage != null) {
            val prefixCandidates = configs.filter { config ->
                val pkg = config.packageName?.takeIf { it.isNotBlank() } ?: return@filter false
                isSegmentSafePackagePrefix(pkg, qualifiedName)
            }
            if (prefixCandidates.isEmpty()) return null
            val longestPrefixLength = prefixCandidates.maxOf { it.packageName!!.length }
            val bestCandidates = prefixCandidates.filter { it.packageName!!.length == longestPrefixLength }
            return when (bestCandidates.size) {
                1 -> bestCandidates.first()
                else -> null
            }
        }

        return null
    }

    fun isMixinListed(
        config: MixinConfigRef,
        mixinClassName: String,
        mixinPackage: String?,
    ): Boolean {
        val qualifiedName = buildMixinQualifiedName(mixinPackage, mixinClassName) ?: return false
        return config.listsMixinRelative(qualifiedName, mixinClassName)
    }

    private fun isUnderDirectory(path: Path, directory: Path): Boolean {
        if (path == directory) {
            return true
        }
        if (path.nameCount <= directory.nameCount) {
            return false
        }
        return path.startsWith(directory)
    }

    private fun buildMixinQualifiedName(mixinPackage: String?, mixinClassName: String?): String? =
        when {
            mixinPackage != null && mixinClassName != null -> "$mixinPackage.$mixinClassName"
            mixinClassName != null -> mixinClassName
            mixinPackage != null -> mixinPackage
            else -> null
        }

    private fun isSegmentSafePackagePrefix(packagePrefix: String, qualifiedName: String): Boolean =
        qualifiedName == packagePrefix || qualifiedName.startsWith("$packagePrefix.")

    private fun parseJsonContent(path: Path, content: String): MixinConfigRef? {
        val root = JsonParser.parseString(normalizeJsonContent(content))
        if (!root.isJsonObject) return null
        val json = root.asJsonObject

        return MixinConfigRef(
            path = path,
            packageName = json.getStringOrNull("package"),
            mixins = json.getStringList("mixins"),
            client = json.getStringList("client"),
            server = json.getStringList("server"),
            common = json.getStringList("common"),
        )
    }

    private fun MixinConfigRef.listsMixinRelative(qualifiedName: String, simpleClassName: String): Boolean {
        val listedEntries = mixins + client + server + common
        return listedEntries.any { entry ->
            resolveListedEntry(entry) == qualifiedName ||
                (packageName.isNullOrBlank() && entry == simpleClassName)
        }
    }

    private fun MixinConfigRef.resolveListedEntry(entry: String): String {
        val pkg = packageName?.takeIf { it.isNotBlank() } ?: return entry
        return "$pkg.$entry"
    }

    private fun stripJson5Comments(source: String): String {
        val out = StringBuilder()
        var index = 0
        var inString = false
        var stringDelimiter = '"'
        while (index < source.length) {
            val char = source[index]
            if (inString) {
                out.append(char)
                if (char == '\\' && index + 1 < source.length) {
                    out.append(source[index + 1])
                    index += 2
                    continue
                }
                if (char == stringDelimiter) {
                    inString = false
                }
                index++
                continue
            }
            when (char) {
                '"', '\'' -> {
                    inString = true
                    stringDelimiter = char
                    out.append(char)
                    index++
                }
                '/' -> if (index + 1 < source.length) {
                    when (source[index + 1]) {
                        '/' -> {
                            index += 2
                            while (index < source.length && source[index] != '\n') {
                                index++
                            }
                        }
                        '*' -> {
                            index += 2
                            while (index + 1 < source.length && !(source[index] == '*' && source[index + 1] == '/')) {
                                index++
                            }
                            index += 2
                        }
                        else -> {
                            out.append(char)
                            index++
                        }
                    }
                } else {
                    out.append(char)
                    index++
                }
                else -> {
                    out.append(char)
                    index++
                }
            }
        }
        return out.toString()
    }

    private fun removeTrailingCommas(json: String): String {
        val out = StringBuilder()
        var index = 0
        var inString = false
        var stringDelimiter = '"'
        while (index < json.length) {
            val char = json[index]
            if (inString) {
                out.append(char)
                if (char == '\\' && index + 1 < json.length) {
                    out.append(json[index + 1])
                    index += 2
                    continue
                }
                if (char == stringDelimiter) {
                    inString = false
                }
                index++
                continue
            }
            if (char == '"' || char == '\'') {
                inString = true
                stringDelimiter = char
                out.append(char)
                index++
                continue
            }
            if (char == ',') {
                var lookahead = index + 1
                while (lookahead < json.length && json[lookahead].isWhitespace()) {
                    lookahead++
                }
                if (lookahead < json.length && (json[lookahead] == '}' || json[lookahead] == ']')) {
                    index++
                    continue
                }
            }
            out.append(char)
            index++
        }
        return out.toString()
    }

    private fun JsonObject.getStringOrNull(key: String): String? {
        val element = get(key) ?: return null
        return if (element.isJsonPrimitive && element.asJsonPrimitive.isString) {
            element.asString
        } else {
            null
        }
    }

    private fun JsonObject.getStringList(key: String): List<String> {
        val element = get(key) ?: return emptyList()
        if (!element.isJsonArray) return emptyList()
        return element.asJsonArray.mapNotNull { item ->
            if (item.isJsonPrimitive && item.asJsonPrimitive.isString) item.asString else null
        }
    }
}
