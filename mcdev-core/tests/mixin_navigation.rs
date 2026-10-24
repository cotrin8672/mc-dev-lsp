use mcdev_core::{
    bytecode::{AtTargetCandidate, BytecodeIndex},
    mixin_navigation::*,
    *,
};
struct Index;
impl ClassIndex for Index {
    fn find_classes(&self, prefix: &str, limit: usize) -> Vec<ClassIndexEntry> {
        let entry = ClassIndexEntry {
            simple_name: "Foo".into(),
            package_name: "target".into(),
            internal_name: "target/Foo".into(),
        };
        if limit > 0 && (entry.simple_name.starts_with(prefix) || entry.fqn().starts_with(prefix)) {
            vec![entry]
        } else {
            vec![]
        }
    }
    fn find_class(&self, name: &str) -> Option<ClassIndexEntry> {
        (name == "target/Foo").then(|| self.find_classes("Foo", 1).remove(0))
    }
    fn find_class_by_fqn(&self, name: &str) -> Option<ClassIndexEntry> {
        (name == "target.Foo").then(|| self.find_classes("Foo", 1).remove(0))
    }
    fn get_methods(&self, _: &str) -> Vec<MethodIndexEntry> {
        vec![]
    }
    fn get_fields(&self, _: &str) -> Vec<FieldIndexEntry> {
        vec![]
    }
}
struct NoCode;
impl BytecodeIndex for NoCode {
    fn get_at_target_candidates(
        &self,
        _: &str,
        _: &str,
        _: Option<&str>,
        _: &str,
    ) -> Vec<AtTargetCandidate> {
        panic!("unexpected bytecode query")
    }
    fn get_return_ordinal_count(&self, _: &str, _: &str, _: Option<&str>) -> usize {
        panic!("unexpected bytecode query")
    }
}
#[test]
fn definition_utf8_offset_and_utf16_position_agree_after_emoji_and_crlf() {
    let source="// 😀 comment\r\nimport org.spongepowered.asm.mixin.Mixin;\r\nimport target.Foo;\r\n@Mixin(Foo.class) class Test {}";
    let offset = source.find("Foo.class").unwrap() + 3;
    let service = MixinDefinitionService::new(&Index, &NoCode);
    let by_offset = service.definitions_at_offset(source, offset, None, None);
    let position = offset_to_position(source, offset).unwrap();
    assert_eq!(
        by_offset,
        service.definitions_at(source, position.line, position.character, None, None)
    );
    assert_eq!(by_offset.len(), 1);
    assert_eq!(by_offset[0].owner_internal_name, "target/Foo");
    let range = by_offset[0].source_range.unwrap();
    assert_eq!(
        range.start,
        McTextPosition {
            line: 3,
            character: 7
        }
    );
    assert_eq!(
        range.end,
        McTextPosition {
            line: 3,
            character: 16
        }
    );
    assert!(by_offset[0].direct_source_document_uri.is_none());
}
#[test]
fn definition_rejects_invalid_utf8_cursor_and_out_of_document_positions() {
    let source = "// 😀\nclass Test {}";
    let service = MixinDefinitionService::new(&Index, &NoCode);
    assert!(service
        .definitions_at_offset(source, source.find('😀').unwrap() + 1, None, None)
        .is_empty());
    assert!(service
        .definitions_at_offset(source, usize::MAX, None, None)
        .is_empty());
    assert!(service
        .definitions_at(source, 100, 0, None, None)
        .is_empty());
}
#[test]
fn reference_ranges_count_utf16_and_keep_source_metadata() {
    let source = "// 😀 comment\r\n@Invoker(\"run\") void callRun() {}";
    let target = McDefinitionTarget {
        kind: MemberKind::Method,
        owner_internal_name: "target/Foo".into(),
        owner_fqn: Some("target.Foo".into()),
        name: Some("run".into()),
        descriptor: None,
        namespace: MappingNamespace::Named,
        source_range: None,
        direct_source_document_uri: None,
    };
    let references = MixinReferenceService::find_references(
        &target,
        &[SourceScanEntry {
            document_uri: "file:///日本語/Test.java".into(),
            text: source.into(),
        }],
    );
    assert_eq!(references.len(), 1);
    assert_eq!(
        references[0].range,
        McTextRange {
            start: McTextPosition {
                line: 1,
                character: 0
            },
            end: McTextPosition {
                line: 1,
                character: 15
            }
        }
    );
    assert_eq!(
        references[0].metadata.get("source").unwrap(),
        "mixin.invoker"
    );
    assert_eq!(references[0].document_uri, "file:///日本語/Test.java");
}
