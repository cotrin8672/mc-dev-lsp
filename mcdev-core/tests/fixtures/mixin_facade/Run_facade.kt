package oracle
import java.io.File
import java.lang.reflect.InvocationTargetException
import java.nio.file.Files
import java.nio.file.Path
import java.util.Comparator

fun main(args:Array<String>){
 Capture.output=File(args[0]);Capture.output.writeText("");val cases=mutableListOf<Any>()
 for(spec in args.drop(2)){
  val className=spec.substringBefore('#');val methodName=spec.substringAfter('#').removeSuffix("()")
  val type=Class.forName(className);val method=type.declaredMethods.single{it.name==methodName}
  val instance=type.getDeclaredConstructor().newInstance();val root=Files.createTempDirectory("mcdev-facade-oracle-").toAbsolutePath().normalize()
  for(field in type.declaredFields.filter{f->f.annotations.any{it.annotationClass.simpleName=="TempDir"}}){field.isAccessible=true;field.set(instance,root)}
  val module=if(className.contains(".jdtls."))"mcdev-jdtls-extension"else"mcdev-core"
  Capture.caseId=module+"/src/test/kotlin/"+className.replace('.','/')+".kt::"+methodName;Capture.legacyClass=className
  fun hooks(names:Set<String>){for(hook in type.declaredMethods.filter{m->m.annotations.any{it.annotationClass.simpleName in names}}){hook.isAccessible=true;hook.invoke(instance)}}
  try{hooks(setOf("BeforeTest","BeforeEach"));method.isAccessible=true;method.invoke(instance);cases+=mapOf("legacyCase" to Capture.caseId,"status" to "passed","calls" to (Capture.counts[Capture.caseId]?:0))}
  catch(error:InvocationTargetException){error.targetException.printStackTrace();cases+=mapOf("legacyCase" to Capture.caseId,"status" to "failed","message" to error.targetException.toString());File(args[1]).writeText(Capture.gson.toJson(cases));throw error.targetException}
  finally{hooks(setOf("AfterTest","AfterEach"));require(root.parent==Path.of(System.getProperty("java.io.tmpdir")).toAbsolutePath().normalize());require(root.fileName.toString().startsWith("mcdev-facade-oracle-"));Files.walk(root).use{paths->paths.sorted(Comparator.reverseOrder()).forEach{p->require(p.toAbsolutePath().normalize().startsWith(root));p.toFile().setWritable(true);Files.delete(p)}}}
 }
 Capture.caseId="";File(args[1]).writeText(Capture.gson.toJson(cases));println("facade cases=${cases.size}; captured calls=${Capture.counts.values.sum()}")
}
