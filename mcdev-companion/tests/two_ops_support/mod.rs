use mcdev_companion::{
    host_facts::{bridge::FactReader, draft::*, FactsError},
    wire::{HandleKind, Registry},
};
use serde_json::{json, Value};
pub const EVALUATION: &str = "current-host-evaluation";
pub const TYPE: &str = "current-type-handle";
pub fn unknown(code: &str) -> Value {
    json!({"state":"unknown","reasons":[{"code":code}]})
}
pub fn ops() -> SupportedOps {
    SupportedOps::intersect(&json!({"capabilities":["semantic-v1",CAPABILITY],"supportedOps":{CAPABILITY:["projectContext","typeJvm"]}})).unwrap()
}
pub fn new_registry() -> Registry {
    let mut r = Registry::default();
    r.register(TYPE, HandleKind::Type).unwrap();
    r
}
pub fn project() -> Value {
    json!({"workspaceUri":{"state":"found","value":"file:///workspace"},"scope":{"state":"found","value":{"module":":app","sourceSet":"main"}},"sourceRoots":{"state":"found","value":["/display/src"]},"outputRoots":{"state":"found","value":["/display/out"]},"classpath":{"state":"found","value":{"entries":[{"id":"origin-b","displayPath":"/display/b.jar"},{"id":"origin-a","displayPath":"/display/a.jar"}],"coverage":{"state":"found","value":null}}},"sdk":{"state":"found","value":{"identity":"selected-sdk-api","api":{"kind":"full"}}}})
}
fn raw(name: &str, desc: &str, flags: u16, attributes: &[&str]) -> Value {
    json!({"name":name,"descriptor":desc,"accessFlags":flags,"signature":null,"attributeNames":attributes})
}
/// Synthetic wire fixture exercises the draft's discriminating facts. It is not
/// claimed as a captured host ClassFileFacts projection or published serializer.
pub fn ty() -> Value {
    json!({"binaryName":{"state":"found","value":{"internalName":"fixture/Model","descriptor":"Lfixture/Model;"}},"classFlags":{"state":"found","value":1},"declaredMembers":{"members":[{"id":"editing-method","name":"value","kind":"method","signature":{"state":"found","value":{"id":"editing-signature","kind":"method","descriptor":"()I"}},"dispatch":{"state":"found","value":"instance"},"visibility":{"state":"found","value":"Public"},"synthetic":{"state":"found","value":false},"mismatches":[]}],"coverage":{"state":"found","value":null}},"abi":{"state":"found","value":{"identity":{"originId":"selected-origin","internalName":"fixture/Model","contentSha256":"a".repeat(64)},"majorVersion":61,"minorVersion":0,"accessFlags":1,"superName":"java/lang/Object","interfaces":["java/io/Serializable"],"signature":"Ljava/lang/Object;Ljava/io/Serializable;","sourceFile":"Model.kt","attributeNames":["Signature","SourceFile","RuntimeVisibleAnnotations","InnerClasses"],"innerClasses":[{"inner":"fixture/Model$Companion","outer":"fixture/Model","name":"Companion","accessFlags":25}],"fields":[raw("name","Ljava/lang/String;",2,&[]),raw("generated","I",0,&["Synthetic"])],"methods":[raw("<init>","(Ljava/lang/String;)V",1,&["Code"]),raw("getName","()Ljava/lang/String;",1,&["Code"]),raw("<clinit>","()V",8,&["Code"]),raw("deserialize","(Lkotlinx/serialization/encoding/Decoder;)Ljava/lang/Object;",0x1041,&["Code"]),raw("deserialize","(Lkotlinx/serialization/encoding/Decoder;)Lfixture/Model;",1,&["Code","RuntimeInvisibleAnnotations"]),raw("value","()I",1,&["Code"])]}},"jvmAbiCoverage":{"state":"found","value":null}})
}
pub struct Reader {
    pub project: Value,
    pub ty: Value,
    pub requests: Vec<Value>,
    pub failure: Option<FactsError>,
    pub extra_fact: bool,
}
impl Reader {
    pub fn new() -> Self {
        Self {
            project: project(),
            ty: ty(),
            requests: vec![],
            failure: None,
            extra_fact: false,
        }
    }
}
impl FactReader for Reader {
    fn read_facts(&mut self, evaluation: &str, queries: &[Value]) -> Result<Value, FactsError> {
        if evaluation != EVALUATION {
            return Err(FactsError::Stale("foreign evaluation".into()));
        }
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.requests
            .push(json!({"evaluation":evaluation,"queries":queries}));
        let mut facts = queries
            .iter()
            .map(|q| match q["op"].as_str() {
                Some("projectContext") => Ok(self.project.clone()),
                Some("typeJvm") if q["target"] == TYPE => Ok(self.ty.clone()),
                _ => Err(FactsError::Protocol(
                    "unnegotiated query or foreign type".into(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.extra_fact {
            facts.push(Value::Null);
        }
        Ok(json!({"facts":facts}))
    }
}
