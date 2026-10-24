// Adapted from the shared source/project Run helpers: original test methods and assertions run unchanged.
package oracle
import java.io.File
import java.lang.reflect.InvocationTargetException
import java.nio.file.Files
import java.nio.file.Path
import java.util.Comparator

fun main(args:Array<String>){
    Capture.output=File(args[0]);Capture.output.writeText("");val cases=mutableListOf<Any>()
    for(spec in args.drop(2)){
        val parts=spec.split('|');val type=Class.forName(parts[0]);val path=parts[1]
        for(method in type.declaredMethods.filter{m->m.annotations.any{it.annotationClass.simpleName=="Test"}}.sortedBy{it.name}){
            val instance=type.getDeclaredConstructor().newInstance()
            val root=Files.createTempDirectory("mcdev-command-oracle-").toAbsolutePath().normalize()
            for(field in type.declaredFields.filter{f->f.annotations.any{it.annotationClass.simpleName=="TempDir"}}){field.isAccessible=true;field.set(instance,root)}
            Capture.caseId=path+"::"+method.name;Capture.legacyClass=parts[0]
            fun hooks(names:Set<String>){for(hook in type.declaredMethods.filter{m->m.annotations.any{it.annotationClass.simpleName in names}}){hook.isAccessible=true;hook.invoke(instance)}}
            try{hooks(setOf("BeforeTest","BeforeEach"));method.isAccessible=true;method.invoke(instance);cases+=mapOf("legacyCase" to Capture.caseId,"legacyClass" to parts[0],"status" to "passed","calls" to (Capture.counts[Capture.caseId]?:0))}
            catch(error:InvocationTargetException){error.targetException.printStackTrace();cases+=mapOf("legacyCase" to Capture.caseId,"status" to "failed","message" to error.targetException.toString());File(args[1]).writeText(Capture.gson.toJson(cases));throw error.targetException}
            finally{
                hooks(setOf("AfterTest","AfterEach"))
                require(root.parent==Path.of(System.getProperty("java.io.tmpdir")).toAbsolutePath().normalize());require(root.fileName.toString().startsWith("mcdev-command-oracle-"))
                Files.walk(root).use{paths->paths.sorted(Comparator.reverseOrder()).forEach{p->require(p.toAbsolutePath().normalize().startsWith(root));p.toFile().setWritable(true);Files.delete(p)}}
            }
        }
    }
    Capture.caseId="";File(args[1]).writeText(Capture.gson.toJson(cases));println("command cases=${cases.size}; captured calls=${Capture.counts.values.sum()}")
}
