package io.github.mcdev.core.mixin

import com.google.gson.Gson
import com.google.gson.GsonBuilder
import com.google.gson.JsonArray
import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import io.github.mcdev.core.project.MixinConfigDiscoveryService

data class MixinConfigEntry(
    val path: String,
    val packageName: String?,
    val mixins: List<String>,
    val client: List<String>,
    val server: List<String>,
    val common: List<String> = emptyList(),
)

data class MixinConfigEditResult(
    val content: String,
    val added: Boolean,
    val arrayName: String,
)

class MixinConfigEditor {
    private val gson: Gson = GsonBuilder().setPrettyPrinting().create()

    fun parse(content: String, path: String = ""): MixinConfigEntry {
        val root = JsonParser.parseString(MixinConfigDiscoveryService.normalizeJsonContent(content))
        return when {
            root.isJsonObject -> parseObject(root.asJsonObject, path)
            root.isJsonArray -> MixinConfigEntry(
                path,
                null,
                root.asJsonArray.mapNotNull { it.asStringOrNull() },
                emptyList(),
                emptyList(),
                emptyList(),
            )
            else -> MixinConfigEntry(path, null, emptyList(), emptyList(), emptyList(), emptyList())
        }
    }

    fun containsEntry(content: String, mixinClassName: String): Boolean {
        val config = parse(content)
        return config.mixins.contains(mixinClassName) ||
            config.client.contains(mixinClassName) ||
            config.server.contains(mixinClassName) ||
            config.common.contains(mixinClassName)
    }

    fun addEntry(
        content: String,
        mixinClassName: String,
        arrayName: String = "mixins",
    ): MixinConfigEditResult {
        val rootElement = JsonParser.parseString(MixinConfigDiscoveryService.normalizeJsonContent(content))
        val root = when {
            rootElement.isJsonObject -> rootElement.asJsonObject
            rootElement.isJsonArray -> {
                val obj = JsonObject()
                obj.add("mixins", rootElement.asJsonArray.deepCopy())
                obj
            }
            else -> JsonObject()
        }
        val array = root.getAsJsonArray(arrayName) ?: JsonArray().also { root.add(arrayName, it) }
        val existing = array.mapNotNull { it.asStringOrNull() }
        if (mixinClassName in existing) {
            return MixinConfigEditResult(content, added = false, arrayName = arrayName)
        }
        val updated = JsonArray()
        (existing + mixinClassName).sorted().forEach { updated.add(it) }
        root.add(arrayName, updated)
        val rendered = gson.toJson(root) + "\n"
        return MixinConfigEditResult(rendered, added = true, arrayName = arrayName)
    }

    fun listMixinClasses(content: String): List<String> {
        val config = parse(content)
        return (config.mixins + config.client + config.server + config.common).distinct().sorted()
    }

    private fun parseObject(obj: JsonObject, path: String): MixinConfigEntry {
        val packageName = obj.get("package")?.asStringOrNull()
        return MixinConfigEntry(
            path = path,
            packageName = packageName,
            mixins = readArray(obj, "mixins"),
            client = readArray(obj, "client"),
            server = readArray(obj, "server"),
            common = readArray(obj, "common"),
        )
    }

    private fun readArray(obj: JsonObject, key: String): List<String> =
        obj.getAsJsonArray(key)?.mapNotNull { it.asStringOrNull() } ?: emptyList()

    private fun JsonElement.asStringOrNull(): String? = if (isJsonPrimitive && asJsonPrimitive.isString) asString else null
}
