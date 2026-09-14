package io.github.mcdev.core.mixinextras

object ConstantAtArgsParser {
    private enum class ConstantDiscriminator {
        INT,
        FLOAT,
        LONG,
        DOUBLE,
        STRING,
        CLASS,
        NULL,
    }

    fun parse(atArgs: List<String>): String? {
        val argsMap = buildArgsMap(atArgs)
        var discriminator: ConstantDiscriminator? = null
        for ((key, rawValue) in argsMap) {
            when (key) {
                "intValue" -> {
                    if (rawValue.toIntOrNull() == null) continue
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.INT) ?: return null
                }
                "floatValue" -> {
                    if (rawValue.toFloatOrNull() == null) continue
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.FLOAT) ?: return null
                }
                "longValue" -> {
                    if (rawValue.toLongOrNull() == null) continue
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.LONG) ?: return null
                }
                "doubleValue" -> {
                    if (rawValue.toDoubleOrNull() == null) continue
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.DOUBLE) ?: return null
                }
                "stringValue" -> {
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.STRING) ?: return null
                }
                "classValue" -> {
                    if (rawValue.isBlank()) continue
                    discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.CLASS) ?: return null
                }
                "nullValue" -> {
                    if (java.lang.Boolean.parseBoolean(rawValue)) {
                        discriminator = mergeDiscriminator(discriminator, ConstantDiscriminator.NULL) ?: return null
                    }
                }
                else -> Unit
            }
        }
        return when (discriminator) {
            ConstantDiscriminator.INT -> "I"
            ConstantDiscriminator.FLOAT -> "F"
            ConstantDiscriminator.LONG -> "J"
            ConstantDiscriminator.DOUBLE -> "D"
            ConstantDiscriminator.STRING -> "Ljava/lang/String;"
            ConstantDiscriminator.CLASS -> "Ljava/lang/Class;"
            ConstantDiscriminator.NULL -> "Ljava/lang/Object;"
            null -> null
        }
    }

    private fun buildArgsMap(atArgs: List<String>): Map<String, String> {
        val map = linkedMapOf<String, String>()
        for (arg in atArgs) {
            val parts = arg.split('=', limit = 2)
            val key = parts[0]
            val value = parts.getOrElse(1) { "" }
            map[key] = value
        }
        return map
    }

    private fun mergeDiscriminator(
        current: ConstantDiscriminator?,
        next: ConstantDiscriminator,
    ): ConstantDiscriminator? = when (current) {
        null -> next
        next -> current
        else -> null
    }
}
