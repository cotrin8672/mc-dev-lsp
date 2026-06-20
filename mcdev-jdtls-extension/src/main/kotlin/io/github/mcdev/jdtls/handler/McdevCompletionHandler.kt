package io.github.mcdev.jdtls.handler

import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.convert.CompletionConvertContext
import io.github.mcdev.jdtls.convert.CompletionItemConverter
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.mixin.SemanticModelCache
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.protocol.ProtocolDecodeException
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.protocol.McdevCompletionRequest
import io.github.mcdev.protocol.McdevCompletionDebugInfo
import io.github.mcdev.protocol.McdevCompletionResponse
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevProtocol
import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.McdevWarning
import java.util.concurrent.ConcurrentHashMap

class McdevCompletionHandler(
    private val projectService: FileBasedProjectContextService = FileBasedProjectContextService(),
    private val mixinFacade: MixinServiceFacade = MixinServiceFacade(),
    private val awAtFacade: AwAtServiceFacade = AwAtServiceFacade(),
    private val decoder: ProtocolPayloadDecoder = ProtocolPayloadDecoder(),
    private val semanticCache: SemanticModelCache = SemanticModelCache { source, documentUri ->
        mixinFacade.semanticModel(source, documentUri)
    },
    private val documentCache: DocumentSnapshotCache = DocumentSnapshotCache(),
) {
    private data class NegativeEntry(
        val reason: String,
        val expiresAtMillis: Long,
    )

    private val negativeCache = ConcurrentHashMap<String, NegativeEntry>()
    private val inFlightLocks = ConcurrentHashMap<String, Any>()

    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevCompletionResponse> =
        try {
            val request = decoder.decodeCompletionRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid completion payload")
        }

    fun handle(request: McdevCompletionRequest): McdevResponseEnvelope<McdevCompletionResponse> {
        val totalStarted = System.nanoTime()
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }
        val requestKey = request.cacheKey()
        val inFlightDedupHit = inFlightLocks.containsKey(requestKey)
        val lock = inFlightLocks.computeIfAbsent(requestKey) { Any() }
        return try {
            synchronized(lock) {
                handleComputed(request, totalStarted, requestKey, inFlightDedupHit)
            }
        } finally {
            inFlightLocks.remove(requestKey, lock)
        }
    }

    private fun handleComputed(
        request: McdevCompletionRequest,
        totalStarted: Long,
        requestKey: String,
        inFlightDedupHit: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse> {
        val loadSessionStarted = System.nanoTime()
        val cachedSession = projectService.loadCachedSession(request.context.workspaceRoot)
        val session = cachedSession.session
        val loadSessionMs = elapsedMs(loadSessionStarted)
        negativeCache[requestKey]
            ?.takeIf { System.currentTimeMillis() <= it.expiresAtMillis }
            ?.let { negative ->
                return emptyCompletion(
                    request = request,
                    reason = negative.reason,
                    totalMs = elapsedMs(totalStarted),
                    loadSessionMs = loadSessionMs,
                    projectSessionCacheHit = cachedSession.cacheHit,
                    projectSessionVersion = cachedSession.version,
                    negativeCacheHit = true,
                    inFlightDedupHit = inFlightDedupHit,
                )
            }
        val awAtFileType = awAtFacade.detectFileType(
            languageId = request.context.languageId,
            documentUri = request.context.documentUri,
        )
        if (awAtFileType != null) {
            val items = awAtFacade.complete(
                session = session,
                source = request.context.bufferText,
                line = request.context.position.line,
                character = request.context.position.character,
                fileType = awAtFileType,
                documentUri = request.context.documentUri,
            )
            return McdevResponseEnvelope(
                capabilities = setOf("completion"),
                result = McdevCompletionResponse(
                    items = CompletionItemConverter.toDtos(
                        items = items,
                        annotationContext = null,
                        source = request.context.bufferText,
                        convertContext = CompletionConvertContext(
                            source = request.context.bufferText,
                            annotationContext = null,
                            mappingResolver = session.context.mappings.resolver,
                            sourceNamespace = session.context.mappings.sourceNamespace,
                            runtimeNamespace = session.context.mappings.runtimeNamespace,
                        ),
                    ),
                ),
            )
        }

        val bufferTextBytes = request.context.bufferText.toByteArray(Charsets.UTF_8).size
        val payloadBytes = estimatePayloadBytes(request)
        val documentSnapshot = documentCache.get(
            documentUri = request.context.documentUri,
            documentVersion = request.context.documentVersion,
            text = request.context.bufferText,
        )
        val options = mixinFacade.toCompletionOptions(
            mixinClassInsert = request.options.mixinClassInsert,
            injectMethodDescriptor = request.options.injectMethodDescriptor,
            preferredAtTarget = request.options.preferredAtTarget,
        )
        val annotationContext = CompletionItemConverter.extractAnnotationContext(
            source = documentSnapshot.snapshot.text,
            line = request.context.position.line,
            character = request.context.position.character,
        )
        val semantic = semanticCache.get(
            source = documentSnapshot.snapshot.text,
            documentUri = documentSnapshot.snapshot.documentUri,
            documentVersion = documentSnapshot.snapshot.version,
        ) { source, documentUri ->
            mixinFacade.semanticModel(source, documentUri, session)
        }
        val completion = mixinFacade.completeWithDebug(
            session = session,
            source = documentSnapshot.snapshot.text,
            line = request.context.position.line,
            character = request.context.position.character,
            options = options,
            documentUri = documentSnapshot.snapshot.documentUri,
            semanticModel = semantic.model,
            languageId = request.context.languageId,
            projectSessionVersion = cachedSession.version,
        )
        completion.debug.zeroItemReason?.let { reason ->
            if (completion.items.isEmpty()) {
                negativeCache[requestKey] = NegativeEntry(
                    reason = reason,
                    expiresAtMillis = System.currentTimeMillis() + negativeTtlMillis(reason),
                )
            }
        }
        val candidateCacheDebug = mixinFacade.candidateCacheDebug()
        val dtoStarted = System.nanoTime()
        val itemDtos = CompletionItemConverter.toDtos(
            items = completion.items,
            annotationContext = annotationContext,
            source = documentSnapshot.snapshot.text,
            convertContext = CompletionConvertContext(
                source = documentSnapshot.snapshot.text,
                annotationContext = annotationContext,
                classInsertMode = options.classInsertMode,
                preferredAtTarget = options.preferredAtTarget,
                mappingResolver = session.context.mappings.resolver,
                sourceNamespace = session.context.mappings.sourceNamespace,
                runtimeNamespace = session.context.mappings.runtimeNamespace,
            ),
        )
        val dtoConvertMs = elapsedMs(dtoStarted)
        return McdevResponseEnvelope(
            capabilities = setOf("completion"),
            result = McdevCompletionResponse(
                items = itemDtos,
                warnings = semantic.model.warnings.map {
                    McdevWarning(code = "MIXIN_PARSE_FALLBACK", message = it)
                } + McdevWarning(
                    code = "MIXIN_PARSE_SOURCE",
                    message = semantic.model.parseSource.name,
                ),
                debug = completion.debug.toProtocolDebug(
                    totalMs = elapsedMs(totalStarted),
                    payloadBytes = payloadBytes,
                    bufferTextBytes = bufferTextBytes,
                    bufferTextFallbackUsed = request.context.bufferTextFallbackUsed,
                    loadSessionMs = loadSessionMs,
                    projectSessionCacheHit = cachedSession.cacheHit,
                    projectSessionVersion = cachedSession.version,
                    documentCacheHit = documentSnapshot.cacheHit,
                    documentSnapshotMs = documentSnapshot.snapshotMs,
                    documentVersion = semantic.documentVersion,
                    semanticCacheHit = semantic.cacheHit,
                    astParseMs = semantic.astParseMs,
                    candidateCacheHit = candidateCacheDebug.hit,
                    candidateBuildMs = candidateCacheDebug.buildMs,
                    dtoConvertMs = dtoConvertMs,
                    negativeCacheHit = false,
                    inFlightDedupHit = inFlightDedupHit,
                ),
            ),
        )
    }

    private fun io.github.mcdev.core.mixin.McdevCompletionDebugInfo.toProtocolDebug(
        totalMs: Long,
        payloadBytes: Int,
        bufferTextBytes: Int,
        bufferTextFallbackUsed: Boolean,
        loadSessionMs: Long,
        projectSessionCacheHit: Boolean,
        projectSessionVersion: Long,
        documentCacheHit: Boolean,
        documentSnapshotMs: Long,
        documentVersion: Long,
        semanticCacheHit: Boolean,
        astParseMs: Long,
        candidateCacheHit: Boolean,
        candidateBuildMs: Long,
        dtoConvertMs: Long,
        negativeCacheHit: Boolean,
        inFlightDedupHit: Boolean,
    ): McdevCompletionDebugInfo =
        McdevCompletionDebugInfo(
            command = command,
            documentUri = documentUri,
            languageId = languageId,
            totalMs = totalMs,
            payloadBytes = payloadBytes,
            bufferTextBytes = bufferTextBytes,
            bufferTextFallbackUsed = bufferTextFallbackUsed,
            projectSessionCacheHit = projectSessionCacheHit,
            projectSessionVersion = projectSessionVersion,
            loadSessionMs = loadSessionMs,
            documentVersion = documentVersion,
            documentCacheHit = documentCacheHit,
            documentSnapshotMs = documentSnapshotMs,
            semanticCacheHit = semanticCacheHit,
            astParseMs = astParseMs,
            parseSource = parseSource?.name,
            parseConfidence = parseConfidence?.name,
            usedCompilationUnit = usedCompilationUnit,
            usedJavaProject = usedJavaProject,
            bindingResolvedCount = bindingResolvedCount,
            bindingFailedCount = bindingFailedCount,
            fallbackReason = fallbackReason,
            semanticContextFound = semanticContextFound,
            fallbackAnnotationContextUsed = fallbackAnnotationContextUsed,
            fallbackAnnotationContextReason = fallbackAnnotationContextReason,
            semanticTargetCount = semanticTargetCount,
            semanticMemberCount = semanticMemberCount,
            completionContextKind = completionContextKind,
            owner = owner,
            methodName = methodName,
            methodDescriptor = methodDescriptor,
            candidateCacheHit = candidateCacheHit,
            candidateBuildMs = candidateBuildMs,
            candidateCountBeforeFilter = candidateCountBeforeFilter,
            candidateCountAfterFilter = candidateCountAfterFilter,
            dtoConvertMs = dtoConvertMs,
            zeroItemReason = zeroItemReason,
            negativeCacheHit = negativeCacheHit,
            inFlightDedupHit = inFlightDedupHit,
            warnings = warnings,
        )

    private fun emptyCompletion(
        request: McdevCompletionRequest,
        reason: String,
        totalMs: Long,
        loadSessionMs: Long,
        projectSessionCacheHit: Boolean,
        projectSessionVersion: Long,
        negativeCacheHit: Boolean,
        inFlightDedupHit: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse> =
        McdevResponseEnvelope(
            capabilities = setOf("completion"),
            result = McdevCompletionResponse(
                items = emptyList(),
                debug = McdevCompletionDebugInfo(
                    command = "mcdev.completion",
                    documentUri = request.context.documentUri,
                    languageId = request.context.languageId,
                    totalMs = totalMs,
                    payloadBytes = estimatePayloadBytes(request),
                    bufferTextBytes = request.context.bufferText.toByteArray(Charsets.UTF_8).size,
                    bufferTextFallbackUsed = request.context.bufferTextFallbackUsed,
                    projectSessionCacheHit = projectSessionCacheHit,
                    projectSessionVersion = projectSessionVersion,
                    loadSessionMs = loadSessionMs,
                    documentVersion = request.context.documentVersion,
                    semanticCacheHit = null,
                    astParseMs = null,
                    parseSource = null,
                    parseConfidence = null,
                    usedCompilationUnit = false,
                    usedJavaProject = false,
                    bindingResolvedCount = 0,
                    bindingFailedCount = 0,
                    fallbackReason = null,
                    semanticContextFound = false,
                    fallbackAnnotationContextUsed = false,
                    semanticTargetCount = 0,
                    semanticMemberCount = 0,
                    completionContextKind = null,
                    owner = null,
                    methodName = null,
                    methodDescriptor = null,
                    candidateCacheHit = null,
                    candidateBuildMs = null,
                    candidateCountBeforeFilter = 0,
                    candidateCountAfterFilter = 0,
                    dtoConvertMs = 0,
                    zeroItemReason = reason,
                    negativeCacheHit = negativeCacheHit,
                    inFlightDedupHit = inFlightDedupHit,
                    warnings = emptyList(),
                ),
            ),
        )

    private fun elapsedMs(started: Long): Long =
        ((System.nanoTime() - started) / 1_000_000).coerceAtLeast(0)

    private fun estimatePayloadBytes(request: McdevCompletionRequest): Int =
        request.context.bufferText.toByteArray(Charsets.UTF_8).size +
            request.context.workspaceRoot.length +
            request.context.documentUri.length +
            request.context.languageId.length +
            request.options.preferredAtTarget.length +
            request.options.mixinClassInsert.length +
            request.options.injectMethodDescriptor.length

    private fun McdevCompletionRequest.cacheKey(): String =
        listOf(
            context.workspaceRoot,
            context.documentUri,
            context.documentVersion ?: context.bufferText.hashCode().toLong(),
            context.position.line,
            context.position.character,
            context.bufferText.hashCode(),
            options.preferredAtTarget,
            options.mixinClassInsert,
            options.injectMethodDescriptor,
        ).joinToString("|")

    private fun negativeTtlMillis(reason: String): Long = when (reason) {
        "NO_COMPLETION_CONTEXT" -> 500
        "NO_MIXIN_TARGET" -> 2_000
        "NO_CANDIDATES" -> 1_000
        "PROJECT_CONTEXT_EMPTY" -> 2_000
        else -> 1_000
    }

    private fun errorEnvelope(code: McdevErrorCode, message: String): McdevResponseEnvelope<McdevCompletionResponse> =
        McdevResponseEnvelope(error = McdevError(code = code, message = message))

    private fun incompleteContext(message: String): McdevResponseEnvelope<McdevCompletionResponse> =
        errorEnvelope(McdevErrorCode.INCOMPLETE_PROJECT_CONTEXT, message)
}
