import io.github.mcdev.core.mapping.*;
import io.github.mcdev.core.model.MappingNamespace;
import com.google.gson.*;
import java.nio.file.*;
import java.util.*;

/** One-time probe of the immutable original compiled Kotlin API. */
public class MappingDuplicateOracle {
    static final MappingNamespace FROM=MappingNamespace.NAMED, TO=MappingNamespace.INTERMEDIARY;
    static Map<MappingNamespace,String> names(String source,String target){Map<MappingNamespace,String> m=new LinkedHashMap<>();m.put(FROM,source);if(target!=null)m.put(TO,target);return m;}
    static Object snapshot(Object result)throws Exception{
        Map<String,Object> out=new LinkedHashMap<>();String kind=result.getClass().getSimpleName();out.put("kind",kind);
        if(kind.equals("Found"))out.put("value",result.getClass().getMethod("getValue").invoke(result));
        else for(String key:List.of("subject","owner","name","descriptor","from","to"))out.put(key,result.getClass().getMethod("get"+Character.toUpperCase(key.charAt(0))+key.substring(1)).invoke(result));
        return out;
    }
    public static void main(String[]args)throws Exception{
        String[][] scenarios={{"identical","B","B"},{"conflicting","B","C"},{"reversed","C","B"},{"firstMissing",null,"C"},{"laterMissing","B",null},{"emptyFirst","","C"}};
        List<Object> rows=new ArrayList<>();
        for(String[] scenario:scenarios){
            List<ClassMapping> classes=new ArrayList<>();List<FieldMapping> fields=new ArrayList<>();List<MethodMapping> methods=new ArrayList<>();
            for(String target:Arrays.asList(scenario[1],scenario[2])){ClassMapping owner=new ClassMapping(names("A",target));classes.add(owner);fields.add(new FieldMapping(owner,"LA;",names("field",target==null?null:"field_"+target)));methods.add(new MethodMapping(owner,"([LA;)LA;",names("method",target==null?null:"method_"+target)));}
            MappingSet set=new MappingSet(List.of(FROM,TO),classes,fields,methods);MappingSetResolver resolver=new MappingSetResolver(set);
            Map<String,Object> row=new LinkedHashMap<>();row.put("case",scenario[0]);row.put("destinations",Arrays.asList(scenario[1],scenario[2]));row.put("className",set.className("A",FROM,TO));row.put("fieldName",set.fieldName("A","field","LA;",FROM,TO));row.put("methodName",set.methodName("A","method","([LA;)LA;",FROM,TO));
            row.put("classResult",snapshot(resolver.remapClass(new ClassRef("A",FROM),TO)));row.put("fieldResult",snapshot(resolver.remapField(new FieldRef("A","field","LA;",FROM),TO)));row.put("methodResult",snapshot(resolver.remapMethod(new MethodRef("A","method","([LA;)LA;",FROM),TO)));row.put("descriptorResult",snapshot(resolver.remapDescriptor("([LA;)LA;",FROM,TO)));rows.add(row);
        }
        Files.writeString(Path.of(args[0]),new GsonBuilder().serializeNulls().setPrettyPrinting().create().toJson(rows));
        System.out.println("original mapping duplicate scenarios: 6, full results: 24");
    }
}
