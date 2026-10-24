use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PROTOCOL_VERSION: i32 = 1;
pub const SERVER_VERSION: &str = "0.1.0";
pub struct McdevProtocol;
impl McdevProtocol {
    pub const VERSION: i32 = PROTOCOL_VERSION;
    pub const SERVER_VERSION: &'static str = SERVER_VERSION;
}
pub struct McdevCommands;
impl McdevCommands {
    pub const COMPLETION: &'static str = "mcdev.completion";
    pub const DEFINITION: &'static str = "mcdev.definition";
    pub const REFERENCES: &'static str = "mcdev.references";
    pub const HOVER: &'static str = "mcdev.hover";
    pub const CODE_ACTION: &'static str = "mcdev.codeAction";
    pub const DIAGNOSTICS: &'static str = "mcdev.diagnostics";
    pub const REINDEX: &'static str = "mcdev.reindex";
    pub const RELOAD_PROJECT_CONTEXT: &'static str = "mcdev.reloadProjectContext";
    pub const DUMP_CONTEXT: &'static str = "mcdev.dumpContext";
    pub const CONTEXT: &'static str = "mcdev.context";
    pub const INFO: &'static str = "mcdev.info";
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    #[serde(rename = "mcdev.completion")]
    Completion,
    #[serde(rename = "mcdev.definition")]
    Definition,
    #[serde(rename = "mcdev.references")]
    References,
    #[serde(rename = "mcdev.hover")]
    Hover,
    #[serde(rename = "mcdev.codeAction")]
    CodeAction,
    #[serde(rename = "mcdev.diagnostics")]
    Diagnostics,
    #[serde(rename = "mcdev.reindex")]
    Reindex,
    #[serde(rename = "mcdev.reloadProjectContext")]
    ReloadProjectContext,
    #[serde(rename = "mcdev.dumpContext")]
    DumpContext,
    #[serde(rename = "mcdev.context")]
    Context,
    #[serde(rename = "mcdev.info")]
    Info,
}
pub const COMMANDS: [Command; 11] = [
    Command::Completion,
    Command::Diagnostics,
    Command::Context,
    Command::Info,
    Command::Reindex,
    Command::ReloadProjectContext,
    Command::DumpContext,
    Command::Definition,
    Command::References,
    Command::Hover,
    Command::CodeAction,
];
impl Command {
    pub fn id(self) -> &'static str {
        match self {
            Self::Completion => McdevCommands::COMPLETION,
            Self::Definition => McdevCommands::DEFINITION,
            Self::References => McdevCommands::REFERENCES,
            Self::Hover => McdevCommands::HOVER,
            Self::CodeAction => McdevCommands::CODE_ACTION,
            Self::Diagnostics => McdevCommands::DIAGNOSTICS,
            Self::Reindex => McdevCommands::REINDEX,
            Self::ReloadProjectContext => McdevCommands::RELOAD_PROJECT_CONTEXT,
            Self::DumpContext => McdevCommands::DUMP_CONTEXT,
            Self::Context => McdevCommands::CONTEXT,
            Self::Info => McdevCommands::INFO,
        }
    }
    pub fn parse(id: &str) -> Option<Self> {
        COMMANDS.into_iter().find(|c| c.id() == id)
    }
    pub fn canonical(self) -> Self {
        if self == Self::Context {
            Self::Diagnostics
        } else {
            self
        }
    }
}

macro_rules! dto {
 ($name:ident {$($(#[$meta:meta])* $field:ident : $ty:ty),* $(,)?})=>{
  #[derive(Debug,Clone,PartialEq,Eq,Serialize,Deserialize)]
  #[serde(rename_all="camelCase")]
  pub struct $name {$($(#[$meta])* pub $field:$ty),*}
 };
}
dto!(McdevClientInfo {
    name: String,
    version: String
});
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct McdevPosition {
    pub line: i32,
    pub character: i32,
}
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct McdevRange {
    pub start: McdevPosition,
    pub end: McdevPosition,
}
dto!(McdevRequestContext{
 protocol_version:i32,workspace_root:String,document_uri:String,language_id:String,position:McdevPosition,buffer_text:String,
 #[serde(default,skip_serializing_if="Option::is_none")] document_version:Option<i64>,
 #[serde(default)] buffer_text_fallback_used:bool,client:McdevClientInfo,
});
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McdevResponseEnvelope<T> {
    #[serde(default = "protocol_version")]
    pub protocol_version: i32,
    #[serde(default = "server_version")]
    pub server_version: String,
    #[serde(default)]
    pub capabilities: BTreeSet<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McdevError>,
}
fn protocol_version() -> i32 {
    PROTOCOL_VERSION
}
fn server_version() -> String {
    SERVER_VERSION.into()
}
impl<T> Default for McdevResponseEnvelope<T> {
    fn default() -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION,
            server_version: SERVER_VERSION.into(),
            capabilities: BTreeSet::new(),
            result: None,
            error: None,
        }
    }
}
impl<T> McdevResponseEnvelope<T> {
    pub fn success(result: T, capabilities: impl IntoIterator<Item = String>) -> Self {
        Self {
            result: Some(result),
            capabilities: capabilities.into_iter().collect(),
            ..Self::default()
        }
    }
    pub fn failure(error: McdevError) -> Self {
        Self {
            error: Some(error),
            ..Self::default()
        }
    }
}
dto!(McdevError{code:McdevErrorCode,message:String,#[serde(default)] details:BTreeMap<String,String>});
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum McdevErrorCode {
    ProtocolMismatch,
    NoApplicableContext,
    IncompleteProjectContext,
    MissingMappingData,
    MissingClasspathEntry,
    BytecodeUnavailable,
    ParseError,
    InternalError,
}
pub fn protocol_mismatch<T>(client_version: i32) -> McdevResponseEnvelope<T> {
    McdevResponseEnvelope::failure(McdevError {
        code: McdevErrorCode::ProtocolMismatch,
        message: format!("protocol mismatch, client={client_version} server={PROTOCOL_VERSION}"),
        details: BTreeMap::from([
            ("client".into(), client_version.to_string()),
            ("server".into(), PROTOCOL_VERSION.to_string()),
        ]),
    })
}
macro_rules! context_request {($($name:ident),* $(,)?)=>{$(dto!($name{context:McdevRequestContext});)*};}
context_request!(
    McdevDefinitionRequest,
    McdevReferencesRequest,
    McdevHoverRequest,
    McdevInfoRequest,
    McdevDiagnosticsRequest,
    McdevReindexRequest,
    McdevReloadProjectContextRequest,
    McdevDumpContextRequest
);
dto!(McdevCompletionRequest {
    context: McdevRequestContext,
    trigger: McdevCompletionTrigger,
    options: McdevCompletionOptions
});
dto!(McdevCompletionTrigger{kind:String,#[serde(default,skip_serializing_if="Option::is_none")] character:Option<String>});
fn smart() -> String {
    "smart".into()
}
fn import() -> String {
    "import".into()
}
fn auto() -> String {
    "auto".into()
}
fn plain_text() -> String {
    "plainText".into()
}
dto!(McdevCompletionOptions {
    #[serde(default = "smart")]
    preferred_at_target: String,
    #[serde(default = "import")]
    mixin_class_insert: String,
    #[serde(default = "auto")]
    inject_method_descriptor: String
});
impl Default for McdevCompletionOptions {
    fn default() -> Self {
        Self {
            preferred_at_target: smart(),
            mixin_class_insert: import(),
            inject_method_descriptor: auto(),
        }
    }
}
dto!(McdevCompletionResponse{items:Vec<McdevCompletionItemDto>,#[serde(default)] is_incomplete:bool,#[serde(default)] warnings:Vec<McdevWarning>,#[serde(default,skip_serializing_if="Option::is_none")] debug:Option<McdevCompletionDebugInfo>});
dto!(McdevCompletionDebugInfo{
 command:String,document_uri:String,language_id:String,
 #[serde(default,skip_serializing_if="Option::is_none")] total_ms:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] payload_bytes:Option<i32>,
 #[serde(default,skip_serializing_if="Option::is_none")] buffer_text_bytes:Option<i32>,
 #[serde(default,skip_serializing_if="Option::is_none")] buffer_text_fallback_used:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] project_session_cache_hit:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] project_session_version:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] load_session_ms:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] document_version:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] document_cache_hit:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] document_snapshot_ms:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] semantic_cache_hit:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] ast_parse_ms:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] parse_source:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] parse_confidence:Option<String>,
 used_compilation_unit:bool,used_java_project:bool,binding_resolved_count:i32,binding_failed_count:i32,
 #[serde(default,skip_serializing_if="Option::is_none")] fallback_reason:Option<String>,
 #[serde(default)] semantic_context_found:bool,#[serde(default)] fallback_annotation_context_used:bool,
 #[serde(default,skip_serializing_if="Option::is_none")] fallback_annotation_context_reason:Option<String>,
 semantic_target_count:i32,semantic_member_count:i32,
 #[serde(default,skip_serializing_if="Option::is_none")] completion_context_kind:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] owner:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] method_name:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] method_descriptor:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] candidate_cache_hit:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] candidate_build_ms:Option<i64>,
 candidate_count_before_filter:i32,candidate_count_after_filter:i32,
 #[serde(default,skip_serializing_if="Option::is_none")] dto_convert_ms:Option<i64>,
 #[serde(default,skip_serializing_if="Option::is_none")] zero_item_reason:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] negative_cache_hit:Option<bool>,
 #[serde(default,skip_serializing_if="Option::is_none")] in_flight_dedup_hit:Option<bool>,warnings:Vec<String>,
});
dto!(McdevCompletionItemDto{
 label:String,#[serde(default,skip_serializing_if="Option::is_none")] detail:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] documentation:Option<String>,
 filter_text:String,insert_text:String,kind:String,sort_key:String,
 #[serde(default,skip_serializing_if="Option::is_none")] edit:Option<McdevTextEdit>,
 #[serde(default)] additional_edits:Vec<McdevTextEdit>,#[serde(default,serialize_with="serialize_nullable_metadata")] metadata:BTreeMap<String,Option<String>>,
 #[serde(default="plain_text")] insert_text_format:String,
});
fn serialize_nullable_metadata<S: serde::Serializer>(
    metadata: &BTreeMap<String, Option<String>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    use serde::ser::SerializeMap;
    let mut map = serializer.serialize_map(None)?;
    for (key, value) in metadata {
        if let Some(value) = value {
            map.serialize_entry(key, value)?;
        }
    }
    map.end()
}
dto!(McdevTextEdit {
    range: McdevRange,
    new_text: String
});
dto!(McdevWarning {
    code: String,
    message: String
});
dto!(McdevCodeActionRequest{context:McdevRequestContext,range:McdevRange,diagnostic_codes:Vec<String>});
dto!(McdevCodeActionResponse{actions:Vec<McdevCodeActionDto>});
dto!(McdevCodeActionDto{title:String,kind:String,edits:Vec<McdevWorkspaceEdit>,#[serde(default)] metadata:BTreeMap<String,String>});
dto!(McdevWorkspaceEdit{document_uri:String,edits:Vec<McdevTextEdit>});
dto!(McdevDefinitionResponse{locations:Vec<McdevLocation>});
dto!(McdevReferencesResponse{locations:Vec<McdevLocation>});
dto!(McdevHoverResponse{contents:Vec<String>,#[serde(default,skip_serializing_if="Option::is_none")] range:Option<McdevRange>});
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum McdevDefinitionResolution {
    Source,
    Jdt,
    BytecodeOnly,
    #[default]
    Unresolved,
}
thread_local! {static DEFINITION_RESOLUTION_ADAPTER:std::cell::Cell<bool>=const{std::cell::Cell::new(false)};}
/// ProtocolPayloadDecoder's Gson registers a lowercase enum adapter; legacy
/// raw handler Gson uses enum names. Keep these serializer contexts distinct.
pub(crate) fn with_definition_resolution_adapter<T>(encode: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            DEFINITION_RESOLUTION_ADAPTER.with(|mode| mode.set(self.0));
        }
    }
    let previous = DEFINITION_RESOLUTION_ADAPTER.with(|mode| mode.replace(true));
    let _reset = Reset(previous);
    encode()
}
impl Serialize for McdevDefinitionResolution {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let raw = match self {
            Self::Source => "SOURCE",
            Self::Jdt => "JDT",
            Self::BytecodeOnly => "BYTECODE_ONLY",
            Self::Unresolved => "UNRESOLVED",
        };
        if DEFINITION_RESOLUTION_ADAPTER.with(std::cell::Cell::get) {
            serializer.serialize_str(&raw.to_ascii_lowercase())
        } else {
            serializer.serialize_str(raw)
        }
    }
}
impl<'de> Deserialize<'de> for McdevDefinitionResolution {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer)?;
        match value.to_ascii_lowercase().as_str() {
            "source" => Ok(Self::Source),
            "jdt" => Ok(Self::Jdt),
            "bytecode_only" => Ok(Self::BytecodeOnly),
            "unresolved" => Ok(Self::Unresolved),
            _ => Err(serde::de::Error::custom(format!(
                "unknown definition resolution: {value}"
            ))),
        }
    }
}
dto!(McdevLocation{
 document_uri:String,range:McdevRange,#[serde(default)] metadata:BTreeMap<String,String>,
 #[serde(default)] resolution:McdevDefinitionResolution,#[serde(default,skip_serializing_if="Option::is_none")] resolution_message:Option<String>,
});
dto!(McdevInfoResponse{
 lines:Vec<String>,#[serde(default,skip_serializing_if="Option::is_none")] build_commit:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] build_time:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] version:Option<String>,
 #[serde(default,skip_serializing_if="Option::is_none")] jar_location:Option<String>,#[serde(default)] registered_commands:Vec<String>,
});
dto!(McdevDiagnosticsResponse{diagnostics:Vec<McdevDiagnosticDto>});
dto!(McdevDiagnosticDto{code:String,severity:String,message:String,range:McdevRange,#[serde(default)] metadata:BTreeMap<String,String>});
dto!(McdevReindexResponse {
    status: String,
    index_state: String,
    classpath_entries: i32
});
dto!(McdevReloadProjectContextResponse{status:String,index_state:String,classpath_entries:i32,lines:Vec<String>});
dto!(McdevDumpContextResponse{
 lines:Vec<String>,project_id:String,root:String,platform:String,mappings:McdevMappingContextDto,classpath:McdevClasspathDto,
 source_sets:Vec<McdevSourceSetDto>,mixin_configs:Vec<String>,access_wideners:Vec<String>,access_transformers:Vec<String>,minecraft_jars:Vec<String>,index_state:String,
});
dto!(McdevMappingContextDto{source_namespace:String,runtime_namespace:String,#[serde(default,skip_serializing_if="Option::is_none")] aw_namespace:Option<String>,#[serde(default,skip_serializing_if="Option::is_none")] at_namespace:Option<String>,available_namespaces:Vec<String>});
dto!(McdevClasspathDto{entry_count:i32,project_outputs:Vec<String>,dependency_jars:Vec<String>,minecraft_jars:Vec<String>,generated_outputs:Vec<String>,captured_at:i64});
dto!(McdevSourceSetDto{name:String,source_directories:Vec<String>,resource_directories:Vec<String>,#[serde(default,skip_serializing_if="Option::is_none")] output_directory:Option<String>});
