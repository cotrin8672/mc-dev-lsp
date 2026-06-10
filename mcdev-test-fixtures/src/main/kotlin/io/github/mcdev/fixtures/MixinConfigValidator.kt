package io.github.mcdev.fixtures

import com.google.gson.JsonObject
import com.google.gson.JsonParser

object MixinConfigValidator {
    data class MixinConfig(
        val required: Boolean,
        val minVersion: String,
        val packageName: String,
        val compatibilityLevel: String,
        val mixins: List<String>,
    )

    fun parse(text: String): MixinConfig {
        val root = JsonParser.parseString(text).asJsonObject
        return MixinConfig(
            required = root.requireBoolean("required"),
            minVersion = root.requireString("minVersion"),
            packageName = root.requireString("package"),
            compatibilityLevel = root.requireString("compatibilityLevel"),
            mixins = root.requireStringArray("mixins"),
        )
    }

    fun validateStructure(text: String): List<String> {
        val errors = mutableListOf<String>()
        val root = runCatching { JsonParser.parseString(text).asJsonObject }
            .getOrElse {
                errors += "invalid JSON: ${it.message ?: it.javaClass.simpleName}"
                return errors
            }

        listOf("required", "minVersion", "package", "compatibilityLevel", "mixins").forEach { key ->
            if (!root.has(key)) {
                errors += "missing required key '$key'"
            }
        }

        if (root.has("mixins") && !root.get("mixins").isJsonArray) {
            errors += "'mixins' must be an array"
        }

        return errors
    }

    private fun JsonObject.requireBoolean(key: String): Boolean {
        val element = get(key) ?: error("missing required key '$key'")
        if (!element.isJsonPrimitive || !element.asJsonPrimitive.isBoolean) {
            error("'$key' must be a boolean")
        }
        return element.asBoolean
    }

    private fun JsonObject.requireString(key: String): String {
        val element = get(key) ?: error("missing required key '$key'")
        if (!element.isJsonPrimitive || !element.asJsonPrimitive.isString) {
            error("'$key' must be a string")
        }
        return element.asString
    }

    private fun JsonObject.requireStringArray(key: String): List<String> {
        val element = get(key) ?: error("missing required key '$key'")
        if (!element.isJsonArray) {
            error("'$key' must be an array")
        }
        return element.asJsonArray.map { entry ->
            if (!entry.isJsonPrimitive || !entry.asJsonPrimitive.isString) {
                error("'$key' entries must be strings")
            }
            entry.asString
        }
    }
}
