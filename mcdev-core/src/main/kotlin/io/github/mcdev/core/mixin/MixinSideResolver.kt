package io.github.mcdev.core.mixin

enum class MixinSide {
    CLIENT,
    SERVER,
}

object MixinSideResolver {
    fun explicitSideFromSource(source: String): MixinSide? {
        val withoutComments = stripComments(source)
        val preambleEnd = findFirstTopLevelTypeKeywordIndex(withoutComments) ?: withoutComments.length
        val preamble = withoutComments.substring(0, preambleEnd)
        return resolveSideFromPreamble(preamble)
    }

    private enum class SideAnnotationKind {
        ENVIRONMENT,
        ONLY_IN,
        SIDE_ONLY,
    }

    private fun resolveSideFromPreamble(preamble: String): MixinSide? {
        val sides = linkedSetOf<MixinSide>()
        var search = 0
        while (search < preamble.length) {
            val at = preamble.indexOf('@', search)
            if (at < 0) break
            val nameEnd = skipAnnotationName(preamble, at)
            if (nameEnd <= at + 1) {
                search = at + 1
                continue
            }
            val qualifiedName = preamble.substring(at + 1, nameEnd)
            if (sideAnnotationKind(qualifiedName) != null) {
                val parenStart = skipWhitespace(preamble, nameEnd)
                if (parenStart >= preamble.length || preamble[parenStart] != '(') {
                    return null
                }
                val parenEnd = findMatchingParen(preamble, parenStart) ?: return null
                val side = parseExplicitSideFromAnnotationBody(preamble.substring(parenStart + 1, parenEnd))
                if (side == null) {
                    search = parenEnd + 1
                    continue
                }
                sides += side
                search = parenEnd + 1
                continue
            }
            search = at + 1
        }
        return when {
            sides.isEmpty() -> null
            MixinSide.CLIENT in sides && MixinSide.SERVER in sides -> null
            else -> sides.single()
        }
    }

    private fun sideAnnotationKind(qualifiedName: String): SideAnnotationKind? =
        when (qualifiedName.substringAfterLast('.')) {
            "Environment" -> if (isFabricEnvironmentAnnotation(qualifiedName)) {
                SideAnnotationKind.ENVIRONMENT
            } else {
                null
            }
            "OnlyIn" -> if (isForgeOnlyInAnnotation(qualifiedName)) {
                SideAnnotationKind.ONLY_IN
            } else {
                null
            }
            "SideOnly" -> if (isLegacySideOnlyAnnotation(qualifiedName)) {
                SideAnnotationKind.SIDE_ONLY
            } else {
                null
            }
            else -> null
        }

    private fun isFabricEnvironmentAnnotation(qualifiedName: String): Boolean =
        qualifiedName == "Environment" || qualifiedName == "net.fabricmc.api.Environment"

    private fun isForgeOnlyInAnnotation(qualifiedName: String): Boolean =
        qualifiedName == "OnlyIn" ||
            qualifiedName == "net.minecraftforge.api.distmarker.OnlyIn" ||
            qualifiedName == "net.neoforged.api.distmarker.OnlyIn"

    private fun isLegacySideOnlyAnnotation(qualifiedName: String): Boolean =
        qualifiedName == "SideOnly" ||
            qualifiedName == "cpw.mods.fml.relauncher.SideOnly" ||
            qualifiedName == "net.minecraftforge.fml.relauncher.SideOnly"

    private fun parseExplicitSideFromAnnotationBody(body: String): MixinSide? {
        val trimmed = body.trim()
        if (trimmed.isEmpty()) {
            return null
        }
        val valuePart = if (trimmed.startsWith("value")) {
            val equalsIndex = trimmed.indexOf('=')
            if (equalsIndex < 0) {
                return null
            }
            trimmed.substring(equalsIndex + 1).trim().trimEnd(',')
        } else {
            trimmed.trimEnd(',')
        }
        return parseQualifiedEnumValue(valuePart.trim())
    }

    private fun parseQualifiedEnumValue(value: String): MixinSide? {
        val normalized = value.replace("\\s".toRegex(), "")
        return when {
            normalized.matches(fabricEnvType("CLIENT")) -> MixinSide.CLIENT
            normalized.matches(fabricEnvType("SERVER")) -> MixinSide.SERVER
            normalized.matches(forgeDist("CLIENT")) -> MixinSide.CLIENT
            normalized.matches(forgeDist("DEDICATED_SERVER")) -> MixinSide.SERVER
            normalized.matches(legacySide("CLIENT")) -> MixinSide.CLIENT
            normalized.matches(legacySide("SERVER")) -> MixinSide.SERVER
            else -> null
        }
    }

    private fun fabricEnvType(constant: String): Regex =
        Regex("(?:net\\.fabricmc\\.api\\.)?EnvType\\.$constant")

    private fun forgeDist(constant: String): Regex =
        Regex("(?:net\\.(?:minecraftforge|neoforged)\\.api\\.distmarker\\.)?Dist\\.$constant")

    private fun legacySide(constant: String): Regex =
        Regex("(?:cpw\\.mods\\.fml\\.relauncher\\.|net\\.minecraftforge\\.fml\\.relauncher\\.)?Side\\.$constant")

    private fun findFirstTopLevelTypeKeywordIndex(source: String): Int? {
        var index = skipLeadingPackageAndImports(source)
        var braceDepth = 0
        var inString = false
        var inChar = false
        var escaped = false

        while (index < source.length) {
            when {
                inString -> {
                    if (escaped) {
                        escaped = false
                    } else when (source[index]) {
                        '\\' -> escaped = true
                        '"' -> inString = false
                    }
                    index++
                }
                inChar -> {
                    if (escaped) {
                        escaped = false
                    } else when (source[index]) {
                        '\\' -> escaped = true
                        '\'' -> inChar = false
                    }
                    index++
                }
                source[index] == '"' -> {
                    inString = true
                    index++
                }
                source[index] == '\'' -> {
                    inChar = true
                    index++
                }
                source[index] == '{' -> {
                    braceDepth++
                    index++
                }
                source[index] == '}' -> {
                    if (braceDepth > 0) {
                        braceDepth--
                    }
                    index++
                }
                braceDepth == 0 && isTypeDeclarationKeyword(source, index) -> return index
                braceDepth == 0 -> index++
                else -> index++
            }
        }
        return null
    }

    private val TYPE_KEYWORD = Regex("""\b(?:class|interface|enum|record)\b""")

    private fun skipLeadingPackageAndImports(source: String): Int {
        var index = 0
        index = skipWhitespace(source, index)
        while (index < source.length) {
            when {
                source.startsWith("package ", index) || source.startsWith("import ", index) -> {
                    val lineEnd = source.indexOf('\n', index).let { if (it < 0) source.length else it + 1 }
                    index = skipWhitespace(source, lineEnd)
                }
                else -> return index
            }
        }
        return index
    }

    private fun isTypeDeclarationKeyword(source: String, index: Int): Boolean {
        val match = TYPE_KEYWORD.find(source, index) ?: return false
        if (match.range.first != index) {
            return false
        }
        var afterKeyword = skipWhitespace(source, match.range.last + 1)
        return afterKeyword < source.length && isJavaIdentifierStart(source[afterKeyword])
    }

    private fun skipWhitespace(source: String, start: Int): Int {
        var index = start
        while (index < source.length && source[index].isWhitespace()) {
            index++
        }
        return index
    }

    private fun isJavaIdentifierStart(ch: Char): Boolean = ch.isLetter() || ch == '_' || ch == '$'

    private fun stripComments(source: String): String {
        val result = StringBuilder(source.length)
        var index = 0
        var inString = false
        var inChar = false
        var escaped = false

        while (index < source.length) {
            when {
                inString -> {
                    result.append(source[index])
                    if (escaped) {
                        escaped = false
                    } else when (source[index]) {
                        '\\' -> escaped = true
                        '"' -> inString = false
                    }
                    index++
                }
                inChar -> {
                    result.append(source[index])
                    if (escaped) {
                        escaped = false
                    } else when (source[index]) {
                        '\\' -> escaped = true
                        '\'' -> inChar = false
                    }
                    index++
                }
                source.startsWith("//", index) -> {
                    while (index < source.length && source[index] != '\n') {
                        result.append(' ')
                        index++
                    }
                }
                source.startsWith("/*", index) -> {
                    index += 2
                    while (index < source.length && !source.startsWith("*/", index)) {
                        result.append(if (source[index] == '\n') '\n' else ' ')
                        index++
                    }
                    if (source.startsWith("*/", index)) {
                        result.append(' ')
                        result.append(' ')
                        index += 2
                    }
                }
                source[index] == '"' -> {
                    inString = true
                    result.append(source[index])
                    index++
                }
                source[index] == '\'' -> {
                    inChar = true
                    result.append(source[index])
                    index++
                }
                else -> {
                    result.append(source[index])
                    index++
                }
            }
        }
        return result.toString()
    }

    private fun skipAnnotationName(source: String, atOffset: Int): Int {
        if (source.getOrNull(atOffset) != '@') {
            return atOffset
        }
        var end = atOffset + 1
        while (end < source.length && isAnnotationNameChar(source[end])) {
            end++
        }
        return end
    }

    private fun isAnnotationNameChar(ch: Char): Boolean =
        ch.isLetterOrDigit() || ch == '_' || ch == '.'

    private fun findMatchingParen(source: String, openIndex: Int): Int? {
        if (source.getOrNull(openIndex) != '(') {
            return null
        }
        var depth = 0
        var inString = false
        var index = openIndex
        while (index < source.length) {
            when {
                inString -> {
                    if (source[index] == '\\') {
                        index += 2
                        continue
                    }
                    if (source[index] == '"') {
                        inString = false
                    }
                }
                source[index] == '"' -> inString = true
                source[index] == '(' -> depth++
                source[index] == ')' -> {
                    depth--
                    if (depth == 0) {
                        return index
                    }
                }
            }
            index++
        }
        return null
    }
}
