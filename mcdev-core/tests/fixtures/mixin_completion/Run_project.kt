package oracle

import java.io.File
import java.lang.reflect.InvocationTargetException
import java.nio.file.Files
import java.nio.file.Path
import java.util.Comparator

fun main(args:Array<String>) {
    Capture.output=File(args[0]);Capture.output.writeText("");val cases=mutableListOf<Any>()
    for(className in args.drop(2)) {
        val type=Class.forName(className)
        for(method in type.declaredMethods.filter{m->m.annotations.any{it.annotationClass.simpleName=="Test"}}.sortedBy{it.name}) {
            val instance=type.getDeclaredConstructor().newInstance();val root=Files.createTempDirectory("mcdev-project-oracle-").toAbsolutePath().normalize();ProjectCapture.root=root
            for(field in type.declaredFields.filter{it.annotations.any{it.annotationClass.simpleName=="TempDir"}}){field.isAccessible=true;field.set(instance,root)}
            Capture.caseId="mcdev-core/src/test/kotlin/"+className.replace('.','/')+".kt::"+method.name;Capture.legacyClass=className
            try{method.isAccessible=true;method.invoke(instance);cases+=mapOf("legacyCase" to Capture.caseId,"legacyClass" to className,"status" to "passed","calls" to (Capture.counts[Capture.caseId]?:0))}
            catch(error:InvocationTargetException){error.targetException.printStackTrace();cases+=mapOf("legacyCase" to Capture.caseId,"status" to "failed","message" to error.targetException.toString());File(args[1]).writeText(Capture.gson.toJson(cases));throw error.targetException}
            finally{require(root.parent==Path.of(System.getProperty("java.io.tmpdir")).toAbsolutePath().normalize());require(root.fileName.toString().startsWith("mcdev-project-oracle-"));Files.walk(root).use{it.sorted(Comparator.reverseOrder()).forEach{path->require(path.toAbsolutePath().normalize().startsWith(root));path.toFile().setWritable(true);Files.delete(path)}}}
        }
    }
    Capture.caseId="";File(args[1]).writeText(Capture.gson.toJson(cases));println("project cases=${cases.size}; captured calls=${Capture.counts.values.sum()}")
}
