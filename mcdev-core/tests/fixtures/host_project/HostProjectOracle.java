import java.lang.reflect.*;
import java.nio.file.*;
import java.util.*;
import com.google.gson.GsonBuilder;

/** Disposable original tests/fixture compiler; never part of the Rust product. */
public class HostProjectOracle {
    static Path fixture;
    static void save(String name, byte[] value) throws Exception {
        Path path=fixture.resolve("classes").resolve(name+".class.bin");
        Files.createDirectories(path.getParent()); Files.write(path,value);
    }
    static Object call(Object obj,String name,Object...args) throws Exception {
        Method method=Arrays.stream(obj.getClass().getDeclaredMethods()).filter(m->m.getName().equals(name)&&m.getParameterCount()==args.length).findFirst().orElseThrow();
        method.setAccessible(true);return method.invoke(obj,args);
    }
    public static void main(String[] args) throws Exception {
        fixture=Path.of(args[0]);
        Object bytecode=Class.forName("io.github.mcdev.jdtls.project.BytecodeIndexAdapterTest").getConstructor().newInstance();
        if(args.length>2&&args[2].equals("outputs")){outputs(bytecode,Path.of(args[1]));return;}
        String[][] methods={
            {"compileClassBytesSamplesClass","test/ClassBytesSamples"},
            {"compileConcurrentSamplesClass","test/ConcurrentSamples"},
            {"compileFieldAccessClass","test/FieldAccessSamples"},
            {"compileConstantSamplesClass","test/ConstantSamples"},
            {"compileInvokeClassificationSamplesClass","test/InvokeClassificationSamples"},
            {"compileOverloadInvokeClass","test/OverloadInvokeSamples"},
            {"compileHierarchyPeerClass","hierarchy/Peer"}};
        for(String[] method:methods)save(method[1],(byte[])call(bytecode,method[0]));
        @SuppressWarnings("unchecked") Map<String,byte[]> hierarchy=(Map<String,byte[]>)call(bytecode,"compileHierarchyFixture");
        for(var entry:hierarchy.entrySet())save(entry.getKey().replace(".class",""),entry.getValue());
        Object lazy=Class.forName("io.github.mcdev.jdtls.project.LazyClasspathClassIndexTest").getConstructor().newInstance();
        save("demo/Alpha",(byte[])call(lazy,"compileClass","Alpha","public int value; public void run() {}"));
        save("demo/Beta",(byte[])call(lazy,"compileClass","Beta","public String name; public void go() {}"));
        save("exact/demo/Alpha",(byte[])call(lazy,"compileClass","Alpha","public void run() {}"));
        for(int i=0;i<5;i++)save("demo/Cache"+i,(byte[])call(lazy,"compileClass","Cache"+i,"public void m"+i+"() {}"));
        String[] classes={"java.JdtClasspathBridgeTest","java.JdtReflectionBridgeTest","project.BytecodeIndexAdapterTest","project.ClasspathClassBytesProviderTest","project.LazyClasspathClassIndexTest","project.McdevProjectSessionTest","project.FileBasedProjectContextServiceTest"};
        List<String> passed=new ArrayList<>();
        for(String suffix:classes){
            Class<?> cls=Class.forName("io.github.mcdev.jdtls."+suffix);
            for(Method test:cls.getDeclaredMethods()){
                if(Arrays.stream(test.getDeclaredAnnotations()).noneMatch(a->a.annotationType().getSimpleName().equals("Test")))continue;
                if(suffix.endsWith("FileBasedProjectContextServiceTest")&&!test.getName().equals("sessionIndexesClasspathClasses"))continue;
                Object object=cls.getConstructor().newInstance();
                for(Field field:cls.getDeclaredFields())if(field.getType()==Path.class){field.setAccessible(true);field.set(object,Files.createTempDirectory("mcdev-host-original-"));}
                try{test.invoke(object);}catch(InvocationTargetException ex){throw new RuntimeException(cls.getName()+"."+test.getName(),ex.getCause());}
                passed.add(suffix+"."+test.getName());
            }
        }
        if(passed.size()!=42)throw new AssertionError("Expected42, got"+passed.size());
        Files.writeString(Path.of(args[1]),new GsonBuilder().setPrettyPrinting().create().toJson(passed));
        System.out.println("original42 assertions passed; exact compiler helper fixtures saved");
    }
    static Object get(Object object,String method) throws Exception{return object.getClass().getMethod(method).invoke(object);}
    static void outputs(Object tests,Path output) throws Exception{
        Path classes=output.getParent().resolve("probe-classes");Files.createDirectories(classes);
        String[][] cases={
            {"test/OverloadInvokeSamples","run","","INVOKE","overloadInvokeClassIndex"},
            {"test/OverloadInvokeSamples","run","()V","INVOKE_ASSIGN","overloadInvokeClassIndex"},
            {"test/OverloadInvokeSamples","run","(I)V","INVOKE","overloadInvokeClassIndex"},
            {"test/FieldAccessSamples","accessFields","()V","FIELD","fieldAccessClassIndex"},
            {"test/ConstantSamples","smallInts","()V","CONSTANT","constantSamplesClassIndex"},
            {"test/InvokeClassificationSamples","mixedAbsCalls","()V","INVOKE","invokeClassificationSamplesClassIndex"},
            {"test/ClassBytesSamples","<init>","()V","INVOKE","emptyClassIndex"},
            {"test/ClassBytesSamples","<init>","()V","RETURN","emptyClassIndex"},
            {"test/ClassBytesSamples","<init>","()V","NEW","emptyClassIndex"},
            {"test/ConstantSamples","smallInts","()V","unknown","constantSamplesClassIndex"}};
        List<Map<String,Object>> rows=new ArrayList<>();
        for(String[] request:cases){
            Path path=classes.resolve(request[0]+".class");Files.createDirectories(path.getParent());Files.copy(fixture.resolve("classes/"+request[0]+".class.bin"),path,StandardCopyOption.REPLACE_EXISTING);
            Class<?> providerType=Class.forName("io.github.mcdev.jdtls.project.ClasspathClassBytesProvider");
            Constructor<?> providerCtor=Arrays.stream(providerType.getConstructors()).filter(c->c.getParameterCount()==4).findFirst().orElseThrow();
            Object provider=providerCtor.newInstance(List.of(classes),Map.of(),256,new java.util.concurrent.atomic.AtomicInteger());
            Object index=request[4].equals("emptyClassIndex")?call(tests,request[4]):call(tests,request[4],request[0]);
            Class<?> adapterType=Class.forName("io.github.mcdev.jdtls.project.BytecodeIndexAdapter");
            Object adapter=Arrays.stream(adapterType.getConstructors()).filter(c->c.getParameterCount()==2).findFirst().orElseThrow().newInstance(provider,index);
            Object result=adapterType.getMethod("getAtTargetCandidates",String.class,String.class,String.class,String.class).invoke(adapter,request[0],request[1],request[2].isEmpty()?null:request[2],request[3]);
            List<Map<String,Object>> candidates=new ArrayList<>();
            for(Object candidate:(List<?>)result){
                Map<String,Object> value=new LinkedHashMap<>();
                for(String field:List.of("owner","name","descriptor","displayLabel","detail","kind","ordinal","namespace","operationKind","instructionOccurrenceIndex","occurrenceResultClassification","conditionOpcode"))value.put(field,get(candidate,"get"+Character.toUpperCase(field.charAt(0))+field.substring(1)));
                Object constant=get(candidate,"getConstantValue");
                if(constant==null)value.put("constantValue",null);else{Map<String,Object> c=new LinkedHashMap<>();c.put("kind",constant.getClass().getSimpleName());c.put("value",constant.getClass().getSimpleName().equals("NullValue")?null:get(constant,constant.getClass().getSimpleName().equals("ClassLiteral")?"getInternalName":"getValue"));value.put("constantValue",c);}
                candidates.add(value);
            }
            Map<String,Object> row=new LinkedHashMap<>();row.put("owner",request[0]);row.put("method",request[1]);row.put("descriptor",request[2].isEmpty()?null:request[2]);row.put("atValue",request[3]);row.put("output",candidates);rows.add(row);
        }
        Files.writeString(output,new GsonBuilder().serializeNulls().setPrettyPrinting().create().toJson(rows));
        System.out.println("10 full public candidate snapshots saved");
    }
}
