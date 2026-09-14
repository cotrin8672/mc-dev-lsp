package io.github.mcdev.core.mixinextras

data class LocalCaptureSnapshot(
    val instructionOccurrenceIndex: Int,
    val candidates: List<LocalCaptureCandidate>,
)

sealed interface LocalCaptureError {
    val message: String
}

data object LocalCaptureMissingClassBytesError : LocalCaptureError {
    override val message: String = "class bytes missing"
}

data object LocalCaptureEmptyClassBytesError : LocalCaptureError {
    override val message: String = "class bytes are empty"
}

data class LocalCaptureCorruptClassBytesError(
    val detail: String,
) : LocalCaptureError {
    override val message: String = "class bytes corrupt: $detail"
}

data class LocalCaptureMethodNotFoundError(
    val methodName: String,
    val methodDescriptor: String,
) : LocalCaptureError {
    override val message: String = "method not found: $methodName$methodDescriptor"
}

data class LocalCaptureAnalysisError(
    val detail: String,
) : LocalCaptureError {
    override val message: String = "bytecode analysis failed: $detail"
}

data class LocalCaptureInvalidInstructionIndexError(
    val instructionOccurrenceIndex: Int,
    val instructionCount: Int,
) : LocalCaptureError {
    override val message: String =
        "invalid instruction occurrence index $instructionOccurrenceIndex (instruction count=$instructionCount)"
}

sealed interface LocalCaptureResult {
    data class Success(val snapshots: List<LocalCaptureSnapshot>) : LocalCaptureResult

    data class Failure(val error: LocalCaptureError) : LocalCaptureResult
}
