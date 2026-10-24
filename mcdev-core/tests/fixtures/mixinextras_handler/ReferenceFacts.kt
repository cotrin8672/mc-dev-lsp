package oracle

import com.google.gson.Gson
import java.io.File

/** One-time VM facts replacing Class.forName in native fixture tests, never a runtime dependency. */
fun main(args: Array<String>) {
    val gson = Gson()
    val names = gson.fromJson(File(args[0]).readText(), Array<String>::class.java).toList()
    val classes = names.associateWith { name -> runCatching { Class.forName(name.replace('/', '.'), false, Thread.currentThread().contextClassLoader) }.getOrNull() }
    val known = classes.mapValues { it.value != null }
    val assignable = linkedMapOf<String, Boolean>()
    for ((fromName, from) in classes) for ((expectedName, expected) in classes) {
        assignable["$fromName|$expectedName"] = from != null && expected != null && from.isAssignableFrom(expected)
    }
    File(args[1]).writeText(gson.toJson(mapOf("known" to known, "assignable" to assignable)))
}
