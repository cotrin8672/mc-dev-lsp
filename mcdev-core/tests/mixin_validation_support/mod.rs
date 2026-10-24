#![allow(dead_code)]
use mcdev_core::{bytecode::*, model::*};
use std::collections::BTreeMap;
pub const OWNER: &str = "net/minecraft/client/MinecraftClient";
pub const DESC: &str = "(Lnet/minecraft/client/gui/screen/Screen;)V";
pub const FD: &str = "Lnet/minecraft/client/gui/screen/Screen;";
pub fn method(name: &str, desc: &str, stat: bool) -> MethodIndexEntry {
    MethodIndexEntry {
        name: name.into(),
        descriptor: desc.into(),
        is_static: stat,
        readable_signature: format!("{name}{desc}"),
    }
}
pub fn field(name: &str, desc: &str, stat: bool) -> FieldIndexEntry {
    FieldIndexEntry {
        name: name.into(),
        descriptor: desc.into(),
        is_static: stat,
        readable_type: if desc == "I" {
            "int".into()
        } else {
            "Screen".into()
        },
    }
}
pub struct Index {
    pub methods: Vec<MethodIndexEntry>,
    pub fields: Vec<FieldIndexEntry>,
}
impl Default for Index {
    fn default() -> Self {
        Self {
            methods: vec![
                method("setScreen", DESC, false),
                method("tick", "()V", false),
                method("render", "(FJZ)V", false),
                method("render", "()V", false),
                method("staticMethod", "()V", true),
                method("<init>", "(I)V", false),
                method("<clinit>", "()V", true),
            ],
            fields: vec![
                field("currentScreen", FD, false),
                field("counter", "I", false),
            ],
        }
    }
}
impl ClassIndex for Index {
    fn find_classes(&self, p: &str, limit: usize) -> Vec<ClassIndexEntry> {
        [
            ClassIndexEntry {
                simple_name: "MinecraftClient".into(),
                package_name: "net.minecraft.client".into(),
                internal_name: OWNER.into(),
            },
            ClassIndexEntry {
                simple_name: "Screen".into(),
                package_name: "net.minecraft.client.gui.screen".into(),
                internal_name: "net/minecraft/client/gui/screen/Screen".into(),
            },
        ]
        .into_iter()
        .filter(|c| c.simple_name.starts_with(p))
        .take(limit)
        .collect()
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50)
            .into_iter()
            .find(|c| c.internal_name == n)
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50).into_iter().find(|c| c.fqn() == n)
    }
    fn get_methods(&self, o: &str) -> Vec<MethodIndexEntry> {
        if o == OWNER {
            self.methods.clone()
        } else {
            vec![]
        }
    }
    fn get_fields(&self, o: &str) -> Vec<FieldIndexEntry> {
        if o == OWNER {
            self.fields.clone()
        } else {
            vec![]
        }
    }
}
pub struct Bytecode;
impl BytecodeIndex for Bytecode {
    fn get_at_target_candidates(
        &self,
        o: &str,
        m: &str,
        d: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        if o != OWNER || m != "tick" || d.is_some_and(|d| d != "()V") {
            return vec![];
        }
        match value {
            "INVOKE" => vec![AtTargetCandidate::new(
                AtTargetKind::Invoke,
                OWNER,
                "setScreen",
                DESC,
            )],
            "FIELD" => vec![AtTargetCandidate::new(
                AtTargetKind::Field,
                OWNER,
                "currentScreen",
                FD,
            )],
            _ => vec![],
        }
    }
    fn get_return_ordinal_count(&self, o: &str, m: &str, d: Option<&str>) -> usize {
        if o == OWNER && m == "tick" && d.is_none_or(|d| d == "()V") {
            2
        } else {
            0
        }
    }
}
pub fn source(body: &str) -> String {
    format!("import net.minecraft.client.MinecraftClient;\nimport net.minecraft.client.gui.screen.Screen;\n@Mixin(MinecraftClient.class)\ninterface M {{\n{body}\n}}")
}
pub fn apply(f: McFix, text: &str) -> String {
    let McFix::WorkspaceEditFix { edits, .. } = f else {
        panic!("workspace fix expected")
    };
    apply_text_edits(text, &edits).unwrap()
}
pub fn diag(code: &str, k: &str, v: &str) -> McDiagnostic {
    McDiagnostic {
        code: code.into(),
        severity: McSeverity::Error,
        message: "test".into(),
        range: McTextRange::default(),
        metadata: BTreeMap::from([(k.into(), v.into())]),
    }
}
/// Exact frozen MixinTestFixtures.kt facts, rather than a reduced substitute.
pub struct LegacyIndex {
    pub methods: BTreeMap<String, Vec<MethodIndexEntry>>,
}
impl Default for LegacyIndex {
    fn default() -> Self {
        let methods = BTreeMap::from([
            (
                OWNER.into(),
                vec![
                    MethodIndexEntry {
                        name: "tick".into(),
                        descriptor: "()V".into(),
                        is_static: false,
                        readable_signature: "tick(): void".into(),
                    },
                    MethodIndexEntry {
                        name: "setScreen".into(),
                        descriptor: DESC.into(),
                        is_static: false,
                        readable_signature: "setScreen(Screen): void".into(),
                    },
                    MethodIndexEntry {
                        name: "render".into(),
                        descriptor: "(FJZ)V".into(),
                        is_static: false,
                        readable_signature: "render(float, long, boolean): void".into(),
                    },
                    MethodIndexEntry {
                        name: "render".into(),
                        descriptor: "(I)V".into(),
                        is_static: false,
                        readable_signature: "render(int): void".into(),
                    },
                ],
            ),
            (
                "net/minecraft/client/font/TextRenderer".into(),
                vec![MethodIndexEntry {
                    name: "draw".into(),
                    descriptor: "(Ljava/lang/String;FFI)I".into(),
                    is_static: false,
                    readable_signature: "draw(String, float, float, int): int".into(),
                }],
            ),
        ]);
        Self { methods }
    }
}
impl ClassIndex for LegacyIndex {
    fn find_classes(&self, p: &str, limit: usize) -> Vec<ClassIndexEntry> {
        [
            ("MinecraftClient", "net.minecraft.client", OWNER),
            (
                "GameRenderer",
                "net.minecraft.client.render",
                "net/minecraft/client/render/GameRenderer",
            ),
            (
                "TextRenderer",
                "net.minecraft.client.font",
                "net/minecraft/client/font/TextRenderer",
            ),
            (
                "DrawContext",
                "net.minecraft.client.gui",
                "net/minecraft/client/gui/DrawContext",
            ),
        ]
        .into_iter()
        .map(
            |(simple_name, package_name, internal_name)| ClassIndexEntry {
                simple_name: simple_name.into(),
                package_name: package_name.into(),
                internal_name: internal_name.into(),
            },
        )
        .filter(|c| {
            c.simple_name.to_lowercase().starts_with(&p.to_lowercase())
                || c.fqn().to_lowercase().starts_with(&p.to_lowercase())
        })
        .take(limit)
        .collect()
    }
    fn find_class(&self, n: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50)
            .into_iter()
            .find(|c| c.internal_name == n)
    }
    fn find_class_by_fqn(&self, n: &str) -> Option<ClassIndexEntry> {
        self.find_classes("", 50).into_iter().find(|c| c.fqn() == n)
    }
    fn get_methods(&self, o: &str) -> Vec<MethodIndexEntry> {
        self.methods.get(o).cloned().unwrap_or_default()
    }
    fn get_fields(&self, o: &str) -> Vec<FieldIndexEntry> {
        if o == OWNER {
            vec![
                FieldIndexEntry {
                    name: "currentScreen".into(),
                    descriptor: FD.into(),
                    is_static: false,
                    readable_type: "Screen".into(),
                },
                FieldIndexEntry {
                    name: "player".into(),
                    descriptor: "Lnet/minecraft/client/player/ClientPlayerEntity;".into(),
                    is_static: false,
                    readable_type: "ClientPlayerEntity".into(),
                },
            ]
        } else {
            vec![]
        }
    }
}
pub struct LegacyBytecode {
    pub counts: BTreeMap<String, usize>,
}
impl Default for LegacyBytecode {
    fn default() -> Self {
        Self {
            counts: BTreeMap::from([("tick".into(), 2)]),
        }
    }
}
impl BytecodeIndex for LegacyBytecode {
    fn get_at_target_candidates(
        &self,
        o: &str,
        m: &str,
        _d: Option<&str>,
        value: &str,
    ) -> Vec<AtTargetCandidate> {
        if o != OWNER || m != "tick" {
            return vec![];
        }
        match value {
            "INVOKE" => {
                let mut c = AtTargetCandidate::new(
                    AtTargetKind::Invoke,
                    "net/minecraft/client/font/TextRenderer",
                    "draw",
                    "(Ljava/lang/String;FFI)I",
                );
                c.display_label = "draw(String, float, float, int): int".into();
                c.detail = "TextRenderer".into();
                c.ordinal = Some(0);
                vec![c]
            }
            "FIELD" => {
                let mut c = AtTargetCandidate::new(AtTargetKind::Field, OWNER, "currentScreen", FD);
                c.display_label = "currentScreen: Screen".into();
                c.detail = "MinecraftClient".into();
                c.ordinal = Some(0);
                vec![c]
            }
            _ => vec![],
        }
    }
    fn get_return_ordinal_count(&self, _o: &str, m: &str, _d: Option<&str>) -> usize {
        self.counts.get(m).copied().unwrap_or(1)
    }
}
