package oracle

import java.io.File
import java.lang.reflect.InvocationTargetException

fun main(args: Array<String>) {
    Capture.output=File(args[0]);Capture.output.writeText("")
    val cases=mutableListOf<Any>()
    for (spec in args.drop(2)) {
        val parts=spec.split('|');val className=parts[0];val path=parts[1]
        val selected=parts.getOrNull(2)?.split(',')?.toSet()
        val type=Class.forName(className)
        for(method in type.declaredMethods.filter{m->m.annotations.any{it.annotationClass.simpleName=="Test"}&&(selected==null||m.name in selected)}.sortedBy{it.name}) {
            val instance=type.getDeclaredConstructor().newInstance()
            Capture.caseId=path+"::"+method.name;Capture.legacyClass=className
            try {
                method.isAccessible=true;method.invoke(instance)
                cases+=mapOf("legacyCase" to Capture.caseId,"legacyClass" to className,"status" to "passed","calls" to (Capture.counts[Capture.caseId]?:0))
            }catch(error:InvocationTargetException){
                error.targetException.printStackTrace()
                cases+=mapOf("legacyCase" to Capture.caseId,"legacyClass" to className,"status" to "failed","message" to error.targetException.toString())
                File(args[1]).writeText(Capture.gson.toJson(cases));throw error.targetException
            }
        }
    }
    Capture.caseId="";File(args[1]).writeText(Capture.gson.toJson(cases))
    println("legacy cases=${cases.size}; captured calls=${Capture.counts.values.sum()}")
}
