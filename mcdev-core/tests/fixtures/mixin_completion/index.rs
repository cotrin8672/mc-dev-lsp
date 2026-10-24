use mcdev_core::*;
use std::collections::BTreeMap;
pub const OWNER: &str = "net/minecraft/client/MinecraftClient";
#[derive(Clone)]
pub struct Index {
    pub classes: Vec<ClassIndexEntry>,
    pub methods: BTreeMap<String, Vec<MethodIndexEntry>>,
    pub fields: BTreeMap<String, Vec<FieldIndexEntry>>,
}
pub fn method(name: &str, descriptor: &str, is_static: bool) -> MethodIndexEntry {
    MethodIndexEntry {
        name: name.into(),
        descriptor: descriptor.into(),
        is_static,
        readable_signature: format!(
            "{name}{}",
            mcdev_core::descriptor::parse_method_descriptor(descriptor)
                .map(|d| mcdev_core::descriptor::render_method(&d))
                .unwrap_or_default()
        ),
    }
}
pub fn field(name: &str, descriptor: &str, is_static: bool) -> FieldIndexEntry {
    FieldIndexEntry {
        name: name.into(),
        descriptor: descriptor.into(),
        is_static,
        readable_type: descriptor.into(),
    }
}
impl Default for Index {
    fn default() -> Self {
        let classes = [
            ("MinecraftClient", "net.minecraft.client", OWNER),
            (
                "Screen",
                "net.minecraft.client.gui.screen",
                "net/minecraft/client/gui/screen/Screen",
            ),
            ("World", "net.minecraft.world", "net/minecraft/world/World"),
        ]
        .into_iter()
        .map(
            |(simple_name, package_name, internal_name)| ClassIndexEntry {
                simple_name: simple_name.into(),
                package_name: package_name.into(),
                internal_name: internal_name.into(),
            },
        )
        .collect();
        Self {
            classes,
            methods: BTreeMap::from([(
                OWNER.into(),
                vec![
                    method("tick", "()V", false),
                    method("render", "(I)V", false),
                    method("render", "(IJ)V", false),
                    method("run", "()V", false),
                ],
            )]),
            fields: BTreeMap::from([(
                OWNER.into(),
                vec![
                    field(
                        "currentScreen",
                        "Lnet/minecraft/client/gui/screen/Screen;",
                        false,
                    ),
                    field("LOGGER", "Ljava/lang/String;", true),
                    field("counter", "I", false),
                ],
            )]),
        }
    }
}
impl ClassIndex for Index {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        self.classes
            .iter()
            .filter(|c| {
                c.simple_name
                    .to_lowercase()
                    .starts_with(&prefix.to_lowercase())
            })
            .take(limit)
            .cloned()
            .collect()
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        self.classes
            .iter()
            .find(|c| c.internal_name == name)
            .cloned()
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        self.classes.iter().find(|c| c.fqn() == name).cloned()
    }
    fn get_methods(&self, owner: &str) -> Vec<MethodIndexEntry> {
        self.methods.get(owner).cloned().unwrap_or_default()
    }
    fn get_fields(&self, owner: &str) -> Vec<FieldIndexEntry> {
        self.fields.get(owner).cloned().unwrap_or_default()
    }
}
