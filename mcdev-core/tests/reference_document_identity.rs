use mcdev_core::{
    mixin_facade::HostMixinFacade, mixin_navigation::reference_document_key, SourceScanEntry,
};

fn source(uri: &str, text: &str) -> SourceScanEntry {
    SourceScanEntry {
        document_uri: uri.into(),
        text: text.into(),
    }
}

#[test]
fn reference_document_keys_decode_file_uris_without_normalizing_opaque_uris() {
    assert_eq!(
        reference_document_key("file:///C:/workspace/fixture/Patch.java"),
        reference_document_key("file:///C%3A/workspace/fixture/Patch.java")
    );
    assert_eq!(
        reference_document_key("file:///tmp/with%20space/../Patch.java"),
        reference_document_key("file://localhost/tmp/Patch.java")
    );
    assert_ne!(
        reference_document_key("file:///tmp/Patch.java"),
        reference_document_key("file:///tmp/Other.java")
    );
    assert_ne!(
        reference_document_key("fixture://C:/Patch.java"),
        reference_document_key("fixture://C%3A/Patch.java")
    );
    assert_ne!(
        reference_document_key("untitled:Patch.java"),
        reference_document_key("file:///tmp/untitled:Patch.java")
    );
    assert_ne!(
        reference_document_key("file:///tmp/bad%uri.java"),
        reference_document_key("file:///tmp/bad%25uri.java")
    );
}

#[test]
fn reference_sources_keep_current_text_and_first_uri_spelling_for_each_file() {
    let current = "file:///C:/workspace/fixture/Patch.java";
    let other = "file:///C%3A/workspace/fixture/Other.java";
    let opaque = "fixture://C:/Patch.java";
    let escaped_opaque = "fixture://C%3A/Patch.java";
    let malformed = "file:///tmp/bad%uri.java";
    let entries = HostMixinFacade::collect_source_entries(
        current,
        "current unsaved text",
        [
            source(
                "file:///C%3A/workspace/fixture/Patch.java",
                "captured alias text",
            ),
            source(other, "first other text"),
            source(
                "file:///C:/workspace/fixture/Other.java",
                "later other alias text",
            ),
            source(opaque, "opaque text"),
            source(escaped_opaque, "different opaque text"),
            source(malformed, "malformed URI text"),
            source(malformed, "duplicate malformed text"),
        ],
    );
    assert_eq!(
        entries,
        [
            source(current, "current unsaved text"),
            source(other, "first other text"),
            source(opaque, "opaque text"),
            source(escaped_opaque, "different opaque text"),
            source(malformed, "malformed URI text")
        ]
    );
}
