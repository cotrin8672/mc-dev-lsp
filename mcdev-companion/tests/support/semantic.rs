//! Explicit normalized host fixtures. No source parser or project index runs in the child.
use super::Process;
use serde_json::{json, Value};
use std::collections::BTreeMap;

pub const TARGET:&str="package fixture; public class Target { public int count; public int shadow$count; public static int total; public int twice(int x) { return x; } public static int global(int x) { return x; } public BlockPos anchor; public BlockState lookup(BlockPos pos) { return null; } public BlockPos[] positions(BlockState state) { return null; } }";
pub fn found(value: Value) -> Value {
    json!({"state":"found","value":value})
}
pub fn unknown() -> Value {
    json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]})
}
pub fn absent() -> Value {
    json!({"state":"definitelyAbsent"})
}
pub fn class_target() -> Value {
    json!({"kind":"class","typeRef":"target-reference"})
}
pub fn string_target() -> Value {
    json!({"kind":"string","value":"fixture.Target"})
}
pub fn arg(name: Option<&str>, value: Value) -> Value {
    json!({"name":name,"value":found(value)})
}

#[derive(Clone)]
pub struct Shadow {
    pub raw: String,
    pub spelling: String,
    pub kind: String,
    pub descriptor: Option<String>,
    pub dispatch: Option<String>,
    pub use_site: Option<String>,
    pub arguments: Vec<Value>,
    pub identity: String,
}
impl Shadow {
    pub fn method(raw: &str, name: &str, descriptor: &str) -> Self {
        Self {
            raw: raw.into(),
            spelling: name.into(),
            kind: "method".into(),
            descriptor: Some(descriptor.into()),
            dispatch: Some("instance".into()),
            use_site: None,
            arguments: vec![],
            identity: "org.spongepowered.asm.mixin.Shadow".into(),
        }
    }
    pub fn field(raw: &str, name: &str, descriptor: &str) -> Self {
        let mut s = Self::method(raw, name, descriptor);
        s.kind = "field".into();
        s
    }
    pub fn static_(mut self) -> Self {
        self.dispatch = Some("static".into());
        self
    }
    pub fn prefix(mut self, value: &str) -> Self {
        self.arguments
            .push(arg(Some("prefix"), json!({"kind":"string","value":value})));
        self
    }
    pub fn unknown_signature(mut self) -> Self {
        self.descriptor = None;
        self
    }
    pub fn unknown_prefix(mut self) -> Self {
        self.arguments
            .push(json!({"name":"prefix","value":unknown()}));
        self
    }
    pub fn aliases(mut self) -> Self {
        self.arguments.push(arg(
            Some("aliases"),
            json!({"kind":"array","values":[arg(None,json!({"kind":"string","value":"other"}))]}),
        ));
        self
    }
    pub fn use_site(mut self, value: &str) -> Self {
        self.use_site = Some(value.into());
        self
    }
}

#[derive(Clone)]
pub struct Case {
    pub source: String,
    pub mixin: String,
    pub mixin_identity: String,
    pub target_arguments: Vec<Value>,
    pub shadows: Vec<Shadow>,
    pub target_fact: Value,
    pub own_fact: Value,
    pub target_members: Vec<Value>,
    pub target_source: String,
}
impl Case {
    pub fn java(annotation: &str, shadows: Vec<Shadow>) -> Self {
        let members = shadows
            .iter()
            .map(|s| s.raw.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let source=format!("package fixture; import org.spongepowered.asm.mixin.Mixin; import org.spongepowered.asm.mixin.Shadow; {annotation} abstract class CaseMixin {{ {members} }}");
        Self::source(source, annotation, shadows)
    }
    pub fn kotlin(shadows: Vec<Shadow>) -> Self {
        let annotation = "@Mixin(Target::class)";
        let members = shadows
            .iter()
            .map(|s| s.raw.as_str())
            .collect::<Vec<_>>()
            .join("\n    ");
        let source=format!("package fixture\nimport org.spongepowered.asm.mixin.Mixin\nimport org.spongepowered.asm.mixin.Shadow\n{annotation}\nabstract class CaseMixin {{\n    {members}\n}}");
        Self::source(source, annotation, shadows)
    }
    pub fn source(source: String, annotation: &str, shadows: Vec<Shadow>) -> Self {
        Self {
            source,
            mixin: annotation.into(),
            mixin_identity: "org.spongepowered.asm.mixin.Mixin".into(),
            target_arguments: vec![arg(None, class_target())],
            shadows,
            target_fact: found(
                json!({"id":"target-type","qualifiedName":"fixture.Target","package":"fixture"}),
            ),
            own_fact: found(
                json!({"id":"own-type","qualifiedName":"fixture.CaseMixin","package":"fixture"}),
            ),
            target_members: target_members(),
            target_source: TARGET.into(),
        }
    }
    pub fn evaluate(&self, p: &mut Process, id: &str) -> Observation {
        let (evaluation, refs, members) = self.snapshot(id);
        p.request(id, "analysis/evaluate", evaluation);
        let mut observation = Observation {
            contributions: Value::Null,
            requirements: vec![],
            bindings: vec![],
            queries: vec![],
        };
        for _ in 0..70 {
            let request = p.receive();
            if request.get("method").is_none() {
                assert_eq!(request["id"], id);
                assert!(
                    request.get("error").is_none(),
                    "child rejected facts: {request}"
                );
                observation.contributions = request["result"].clone();
                return observation;
            }
            assert_eq!(
                request["params"]["evaluation"], id,
                "evaluation-scoped reads"
            );
            let result = match request["method"].as_str().unwrap() {
                "analysis/readFacts" => {
                    assert!(
                        request["params"]["queries"].as_array().unwrap().len() <= 256,
                        "host fact batch bound"
                    );
                    let facts = request["params"]["queries"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|q| {
                            observation.queries.push(q.clone());
                            match q["op"].as_str().unwrap() {
                                "resolveTypeRef" => refs
                                    .get(q["reference"].as_str().unwrap())
                                    .expect("host never accepts foreign type-reference handles")
                                    .clone(),
                                "resolveType" => {
                                    assert_eq!(q["name"], "fixture.Target");
                                    self.target_fact.clone()
                                }
                                "declarationMember" => {
                                    let member = members
                                        .get(q["declaration"].as_str().unwrap())
                                        .expect("host never accepts foreign declarations");
                                    if self.own_fact["state"] != "found"
                                        || member["value"]["signature"]["state"] != "found"
                                        || member["value"]["dispatch"]["state"] != "found"
                                    {
                                        unknown()
                                    } else {
                                        member.clone()
                                    }
                                }
                                other => panic!("unexpected query {other}"),
                            }
                        })
                        .collect::<Vec<_>>();
                    json!({"facts":facts})
                }
                "analysis/require" => {
                    assert!(
                        request["params"]["requirements"].as_array().unwrap().len() <= 256,
                        "host requirement batch bound"
                    );
                    let results=request["params"]["requirements"].as_array().unwrap().iter().map(|requirement|{
                        let authored=requirement["authored"].as_str().unwrap();let member=members.get(authored).unwrap();
                        assert_eq!(requirement["signature"],member["value"]["signature"]);assert_eq!(requirement["dispatch"],member["value"]["dispatch"]);assert_eq!(requirement["kind"],member["value"]["kind"]);
                        let binding=self.binding(requirement);
                        observation.requirements.push(requirement.clone());observation.bindings.push(binding.clone());
                        json!({"id":format!("{id}:requirement:{}",observation.requirements.len()),"binding":binding,"candidates":[]})
                    }).collect::<Vec<_>>();
                    json!({"requirements":results})
                }
                other => panic!("unexpected child method {other}"),
            };
            p.send(json!({"jsonrpc":"2.0","id":request["id"],"result":result}));
        }
        panic!("bounded mock-host batch count exceeded")
    }
    fn binding(&self, r: &Value) -> Value {
        if r["target"]["state"] == "unknown"
            || r["signature"]["state"] == "unknown"
            || r["dispatch"]["state"] == "unknown"
        {
            return unknown();
        }
        if r["target"]["state"] == "definitelyAbsent" {
            return absent();
        }
        let matches = self
            .target_members
            .iter()
            .filter(|m| {
                m["name"] == r["names"]["values"][0]
                    && m["kind"] == r["kind"]
                    && m["descriptor"] == r["signature"]["value"]["descriptor"]
                    && m["dispatch"] == r["dispatch"]["value"]
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [member] => found(json!({"target":member["id"]})),
            [] => absent(),
            _ => unknown(),
        }
    }
    pub fn snapshot(&self, id: &str) -> (Value, BTreeMap<String, Value>, BTreeMap<String, Value>) {
        let source = &self.source;
        let owner_start = source.find("class CaseMixin").unwrap() + "class ".len();
        let mut refs = BTreeMap::new();
        let mut members = BTreeMap::new();
        refs.insert("mixin-reference".into(),found(json!({"id":format!("{id}:annotation-mixin"),"qualifiedName":self.mixin_identity,"package":"org.spongepowered.asm.mixin"})));
        refs.insert("target-reference".into(), self.target_fact.clone());
        refs.insert("extra-target-reference".into(),found(json!({"id":format!("{id}:extra-target"),"qualifiedName":"fixture.BlockPos","package":"fixture"})));
        let start = source.find(&self.mixin).unwrap();
        let range = json!({"start":start,"end":start+self.mixin.len()});
        let arguments = self
            .target_arguments
            .iter()
            .map(|a| argument(a.clone(), source, &range))
            .collect::<Vec<_>>();
        let mut declarations = vec![
            json!({"id":"owner-declaration","owner":null,"range":{"start":owner_start,"end":owner_start+9},"name":"CaseMixin","annotations":[{"typeRef":"mixin-reference","useSiteTarget":null,"range":range,"arguments":arguments}]}),
        ];
        for (n, shadow) in self.shadows.iter().enumerate() {
            let raw_start = source.find(&shadow.raw).unwrap();
            let start = raw_start + shadow.raw.rfind(&shadow.spelling).unwrap();
            let end = start + shadow.spelling.len();
            let annotation_end = raw_start + shadow.raw.find(' ').unwrap_or(shadow.raw.len());
            let range = json!({"start":raw_start,"end":annotation_end});
            let arguments = shadow
                .arguments
                .iter()
                .map(|a| argument(a.clone(), source, &range))
                .collect::<Vec<_>>();
            let declaration = format!("member-declaration-{n}");
            let reference = format!("shadow-reference-{n}");
            refs.insert(reference.clone(),found(json!({"id":format!("{id}:annotation-shadow-{n}"),"qualifiedName":shadow.identity,"package":"org.spongepowered.asm.mixin"})));
            declarations.push(json!({"id":declaration,"owner":"owner-declaration","range":{"start":start,"end":end},"name":shadow.spelling,"annotations":[{"typeRef":reference,"useSiteTarget":shadow.use_site,"range":range,"arguments":arguments}]}));
            let signature = shadow.descriptor.as_ref().map_or_else(unknown, |d| {
                found(json!({"id":format!("{id}:signature-{n}"),"kind":shadow.kind,"descriptor":d}))
            });
            let dispatch = shadow
                .dispatch
                .as_ref()
                .map_or_else(unknown, |d| found(json!(d)));
            members.insert(declaration,found(json!({"id":format!("{id}:member-{n}"),"name":shadow.spelling.trim_matches('`'),"kind":shadow.kind,"signature":signature,"dispatch":dispatch,"visibility":found(json!("Public")),"synthetic":found(json!(false)),"mismatches":[]})));
        }
        (
            json!({"evaluation":id,"owner":"CaseMixin.unsaved","source":source,"configuration":{},"declarations":declarations}),
            refs,
            members,
        )
    }
}
fn argument(mut a: Value, source: &str, range: &Value) -> Value {
    a["range"] = range.clone();
    if a["value"]["state"] == "found" {
        let v = &mut a["value"]["value"];
        match v["kind"].as_str().unwrap() {
            "string" => {
                let text = v["value"].as_str().unwrap();
                let start = if text.is_empty() {
                    source.find("\"\"").unwrap() + 1
                } else {
                    source.find(text).unwrap()
                };
                v["contentRange"] = json!({"start":start,"end":start+text.len()});
            }
            "array" => {
                for item in v["values"].as_array_mut().unwrap() {
                    *item = argument(item.clone(), source, range);
                }
            }
            _ => (),
        }
    }
    a
}
pub struct Observation {
    pub contributions: Value,
    pub requirements: Vec<Value>,
    pub bindings: Vec<Value>,
    pub queries: Vec<Value>,
}
impl Observation {
    pub fn bound(&self) -> usize {
        self.bindings
            .iter()
            .filter(|b| b["state"] == "found")
            .count()
    }
    pub fn incomplete(&self) -> bool {
        self.contributions["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| {
                d["severity"] == "hint"
                    && d["message"]
                        .as_str()
                        .unwrap()
                        .starts_with("[mcdev.mixin.incomplete]")
            })
    }
    pub fn missing(&self) -> bool {
        self.contributions["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| {
                d["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("[mcdev.mixin.shadow.missing]")
            })
    }
}
pub fn target_members() -> Vec<Value> {
    [("count","field","I","instance"),("shadow$count","field","I","instance"),("total","field","I","static"),("twice","method","(I)I","instance"),("global","method","(I)I","static"),("anchor","field","Lfixture/BlockPos;","instance"),("lookup","method","(Lfixture/BlockPos;)Lfixture/BlockState;","instance"),("positions","method","(Lfixture/BlockState;)[Lfixture/BlockPos;","instance")].into_iter().enumerate().map(|(n,(name,kind,descriptor,dispatch))|json!({"id":format!("target-member-{n}"),"name":name,"kind":kind,"descriptor":descriptor,"dispatch":dispatch})).collect()
}
