//! Test-only host producer/typed-reader proof; this is not a product command.
use mcdev_companion::{
    host_facts::{
        inputs::{InputOp, InputOps, RuntimeAuthority, RuntimeDescriptor, ScopedInputs},
        FactsError,
    },
    provider_commands::{run_with_command_handler, CommandFacts},
    wire::{Evaluation, Fact, Registry},
};
use mcdev_core::expression_engine::{
    decode_method, Cancellation, CancellationChecker, IdentifierPool, InstructionOperand,
    MatchContext, MatchContextType,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{io, sync::Arc};

const OWNER: &str = "com/example/target/SimpleTarget";
const METHOD: &str = "(Ljava/lang/String;FF)V";
const SCOPE: &str = "test provider + real host + production Rust typed reader; no product Local completion or full Sugar parity";

struct Active(CommandFacts);
impl CancellationChecker for Active {
    fn check_cancelled(&self) -> Result<(), Cancellation> {
        self.0.check_active().map_err(|error| {
            Cancellation(Arc::new(io::Error::new(io::ErrorKind::Interrupted, error)))
        })
    }
}

fn found<T>(fact: Fact<T>, operation: &str) -> Result<T, FactsError> {
    match fact {
        Fact::Found { value } => Ok(value),
        Fact::Unknown { reasons } => Err(FactsError::BytecodeUnavailable(format!(
            "{operation} is Unknown: {}",
            json!(reasons)
        ))),
        Fact::DefinitelyAbsent => Err(FactsError::BytecodeUnavailable(format!(
            "{operation} did not provide runtime bytes"
        ))),
    }
}

fn require_code(descriptor: &RuntimeDescriptor) -> Result<(), FactsError> {
    if descriptor.authority != RuntimeAuthority::RuntimeBytecode || !descriptor.code_available {
        return Err(FactsError::BytecodeUnavailable(
            "header-only class facts cannot prove Code or LVT".into(),
        ));
    }
    Ok(())
}

fn inspect(bytes: &[u8], cancellation: &dyn CancellationChecker) -> Result<Value, FactsError> {
    let pool = IdentifierPool::default();
    let context = MatchContext {
        context_type: MatchContextType::Custom,
        identifier_pool: &pool,
        allow_incomplete_list_inputs: false,
        common_super_class: &|_, _| None,
        cancellation,
    };
    let method = decode_method(bytes, "draw", METHOD, &context)
        .map_err(|error| FactsError::Cancelled(error.to_string()))?
        .map_err(|error| {
            FactsError::BytecodeUnavailable(format!("draw Code unavailable: {error:?}"))
        })?;
    if method.class_name != OWNER || method.is_static || method.instructions.is_empty() {
        return Err(FactsError::BytecodeUnavailable(
            "wrong owner or missing instance Code".into(),
        ));
    }
    let local = method
        .local_variables
        .iter()
        .find(|local| {
            local.name == "text"
                && local.slot == 1
                && local.descriptor == "Ljava/lang/String;"
                && local.start < local.end
        })
        .ok_or_else(|| {
            FactsError::BytecodeUnavailable("text slot 1 String LVT is unavailable".into())
        })?;
    let invoke = method
        .instructions
        .iter()
        .find(|instruction| {
            instruction.opcode == 0xb6
                && matches!(&instruction.operand,
            InstructionOperand::Member { owner, name, descriptor, .. }
                if owner == "java/lang/String" && name == "length" && descriptor == "()I")
        })
        .ok_or_else(|| {
            FactsError::BytecodeUnavailable("String.length invocation is unavailable".into())
        })?;
    Ok(json!({
        "owner":method.class_name,"name":method.method_name,"descriptor":method.descriptor,
        "instructionCount":method.instructions.len(),"codeAvailable":true,
        "local":{"name":local.name,"slot":local.slot,"descriptor":local.descriptor,
            "start":local.start,"end":local.end},
        "stringLengthInvoke":{"opcode":invoke.opcode,"byteOffset":invoke.byte_offset,
            "owner":"java/lang/String","name":"length","descriptor":"()I"}
    }))
}

fn probe(params: &Value, initialization: &Value, facts: CommandFacts) -> Result<Value, FactsError> {
    if params["command"] != "mcdev.info" {
        return Err(FactsError::Protocol(
            "test provider accepts only mcdev.info".into(),
        ));
    }
    let evaluation: Evaluation = serde_json::from_value(params.clone())
        .map_err(|error| FactsError::Protocol(error.to_string()))?;
    evaluation
        .validate(&mut Registry::default())
        .map_err(FactsError::Protocol)?;
    let operations = InputOps::intersect(initialization)?;
    if !operations.supports(InputOp::RuntimeClass) || !operations.supports(InputOp::ClassBytes) {
        return Err(FactsError::BytecodeUnavailable(
            "runtimeClass/classBytes were not negotiated".into(),
        ));
    }
    let mut inputs = ScopedInputs::new(&evaluation.evaluation, &operations, facts.clone())?;
    let class = found(inputs.runtime_class(OWNER)?, "runtimeClass")?;
    require_code(&class.descriptor)?;
    let bytes = found(inputs.class_bytes(&class)?, "classBytes")?;
    let method = inspect(&bytes, &Active(facts.clone()))?;
    facts.check_active()?;
    Ok(json!({
        "testOnly":true,"scope":SCOPE,"probe":"runtime-bytes-code-lvt","probePid":std::process::id(),
        "evaluation":evaluation.evaluation,"owner":evaluation.owner,
        "documentUri":params["context"]["documentUri"],"documentVersion":params["context"]["documentVersion"],
        "sourceSha256":format!("{:x}", Sha256::digest(evaluation.source.as_bytes())),
        "negotiated":initialization,"runtime":class.descriptor,"method":method,
        "byteLength":bytes.len(),"sha256":format!("{:x}", Sha256::digest(&bytes)),"bytes":bytes
    }))
}

fn main() -> io::Result<()> {
    run_with_command_handler(
        io::BufReader::new(io::stdin()),
        io::stdout(),
        |params, initialization, facts| match probe(&params, &initialization, facts) {
            Err(
                error @ (FactsError::Cancelled(_) | FactsError::Stale(_) | FactsError::Protocol(_)),
            ) => Err(error),
            Err(error) => Ok(
                json!({"testOnly":true,"scope":SCOPE,"probe":"runtime-bytes-code-lvt","probePid":std::process::id(),
                "error":FactsError::BytecodeUnavailable(error.to_string()).protocol()}),
            ),
            result => result,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcdev_core::expression_engine::NO_CANCELLATION;

    #[test]
    fn native_decoder_preserves_original_target_code_and_local_metadata() {
        let bytes = include_bytes!(
            "../../mcdev-core/tests/fixtures/mixin_completion/project_files/SimpleTarget.class.bin"
        );
        let result = inspect(bytes, &NO_CANCELLATION).unwrap();
        assert_eq!(result["descriptor"], METHOD);
        assert_eq!(result["local"]["name"], "text");
        assert_eq!(result["local"]["slot"], 1);
        assert_eq!(result["local"]["descriptor"], "Ljava/lang/String;");
        assert_eq!(result["stringLengthInvoke"]["opcode"], 0xb6);
    }

    #[test]
    fn unknown_and_header_only_facts_cannot_become_code_proof() {
        let unknown = || json!({"state":"unknown","reasons":[{"code":"MissingFacts"}]});
        assert!(matches!(
            found::<Vec<u8>>(serde_json::from_value(unknown()).unwrap(), "classBytes"),
            Err(FactsError::BytecodeUnavailable(_))
        ));
        let header: RuntimeDescriptor = serde_json::from_value(json!({
            "id":"header","internalName":OWNER,"originId":"sdk","contentSha256":"0".repeat(64),
            "byteLength":0,"authority":"sdk-api-header","codeAvailable":false,"runtimeOnly":false,
            "classHeader":unknown(),"hierarchy":unknown(),"runtimeAbiCoverage":unknown(),"sourceCurrent":unknown()
        })).unwrap();
        assert!(matches!(
            require_code(&header),
            Err(FactsError::BytecodeUnavailable(_))
        ));
        assert!(matches!(
            inspect(b"not a class", &NO_CANCELLATION),
            Err(FactsError::BytecodeUnavailable(_))
        ));
    }
}
