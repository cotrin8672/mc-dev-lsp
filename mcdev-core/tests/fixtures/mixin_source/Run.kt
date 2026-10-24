package oracle
import java.io.File
import java.lang.reflect.InvocationTargetException

fun main(args: Array<String>) {
    Capture.output = File(args[0]); Capture.output.writeText("")
    val cases = mutableListOf<Any>()
    for (arg in args.drop(2)) {
        val className = arg.substringBefore('|')
        val path = arg.substringAfter('|', "mcdev-core/src/test/kotlin/" + className.replace('.','/') + ".kt")
        val type = Class.forName(className)
        for(method in type.declaredMethods.filter { m -> m.annotations.any { it.annotationClass.simpleName == "Test" } }.sortedBy { it.name }) {
            val instance = type.getDeclaredConstructor().newInstance()
            Capture.caseId = path + "::" + method.name
            Capture.legacyClass = className
            try {
                method.isAccessible = true; method.invoke(instance)
                val calls = Capture.counts[Capture.caseId] ?: 0
                cases += mapOf("legacyCase" to Capture.caseId, "status" to "passed", "calls" to calls)
            } catch(error: InvocationTargetException) {
                error.targetException.printStackTrace()
                cases += mapOf("legacyCase" to Capture.caseId, "status" to "failed", "message" to error.targetException.toString())
                File(args[1]).writeText(Capture.gson.toJson(cases)); throw error.targetException
            }
        }
    }
    Capture.caseId = ""
    File(args[1]).writeText(Capture.gson.toJson(cases))
    println("legacy cases=${cases.size}; captured calls=${Capture.counts.values.sum()}; summary=${args[1]}")
}
