package io.github.mcdev.jdtls.handler

import io.github.mcdev.core.at.AtContextExtractor
import io.github.mcdev.core.aw.AwContextExtractor
import io.github.mcdev.core.awat.AwAtFileType
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.jdtls.awat.AwAtServiceFacade
import io.github.mcdev.jdtls.convert.CompletionConvertContext
import io.github.mcdev.jdtls.convert.CompletionItemConverter
import io.github.mcdev.jdtls.convert.CompletionReplacementRange
import io.github.mcdev.jdtls.mixin.BufferOnlyCompletionResult
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
import java.util.LinkedHashMap
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
    private val currentTimeMillis: () -> Long = { System.currentTimeMillis() },
) {
    private val negativeCache = NegativeCompletionCache(currentTimeMillis = currentTimeMillis)
    private val inFlightLocks = ConcurrentHashMap<String, Any>()

    internal fun negativeCacheSizeForTests(): Int = negativeCache.sizeForTests()

    fun handle(arguments: List<Any?>): McdevResponseEnvelope<McdevCompletionResponse> =
        try {
            val request = decoder.decodeCompletionRequest(arguments)
            handle(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid completion payload")
        }

    /**
     * Handles the completion request on the same-JVM project-aware transport.
     *
     * The normal command path deliberately returns buffer-only results for the
     * contexts that can be answered without a project session. The transport
     * must always compute the final project-aware result instead of relabeling
     * that provisional subset as final.
     */
    internal fun handleProjectAware(arguments: List<Any?>): McdevResponseEnvelope<McdevCompletionResponse> =
        try {
            val request = decoder.decodeCompletionRequest(arguments)
            handleProjectAware(request)
        } catch (error: ProtocolDecodeException) {
            errorEnvelope(McdevErrorCode.PARSE_ERROR, error.message ?: "invalid completion payload")
        }

    fun handle(request: McdevCompletionRequest): McdevResponseEnvelope<McdevCompletionResponse> =
        handle(request, projectAware = false)

    internal fun handleProjectAware(request: McdevCompletionRequest): McdevResponseEnvelope<McdevCompletionResponse> =
        handle(request, projectAware = true)

    private fun handle(
        request: McdevCompletionRequest,
        projectAware: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse> {
        val totalStarted = System.nanoTime()
        if (request.context.protocolVersion != McdevProtocol.VERSION) {
            return typedProtocolMismatch(request.context.protocolVersion)
        }
        if (request.context.workspaceRoot.isBlank()) {
            return incompleteContext("workspace root is required")
        }
        if (
            projectAware &&
            awAtFacade.detectFileType(request.context.languageId, request.context.documentUri) == null &&
            !mixinFacade.hasJavaProject(request.context.documentUri)
        ) {
            return incompleteContext("JDT project dependencies are not available yet")
        }
        val requestKey = request.cacheKey()
        val inFlightDedupHit = inFlightLocks.containsKey(requestKey)
        val lock = inFlightLocks.computeIfAbsent(requestKey) { Any() }
        return try {
            synchronized(lock) {
                handleComputed(request, totalStarted, requestKey, inFlightDedupHit, projectAware)
            }
        } finally {
            inFlightLocks.remove(requestKey, lock)
        }
    }

    /**
     * Computes only the no-index part of completion.
     *
     * A standalone stdio helper uses this before the normal JDT LS command. A
     * null result means that the request needs the project-aware path.
     */
    internal fun handleBufferOnly(request: McdevCompletionRequest): McdevResponseEnvelope<McdevCompletionResponse>? {
        if (request.context.protocolVersion != McdevProtocol.VERSION || request.context.workspaceRoot.isBlank()) {
            return null
        }
        return bufferOnlyResponse(
            request = request,
            totalStarted = System.nanoTime(),
            inFlightDedupHit = false,
            requireItems = true,
        )
    }

    private fun handleComputed(
        request: McdevCompletionRequest,
        totalStarted: Long,
        requestKey: String,
        inFlightDedupHit: Boolean,
        projectAware: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse> {
        if (!projectAware) {
            negativeCache.get(requestKey)?.let { reason ->
                return emptyCompletion(
                    request = request,
                    reason = reason,
                    totalMs = elapsedMs(totalStarted),
                    loadSessionMs = 0,
                    projectSessionCacheHit = false,
                    projectSessionVersion = 0,
                    negativeCacheHit = true,
                    inFlightDedupHit = inFlightDedupHit,
                )
            }
            bufferOnlyResponse(
                request = request,
                totalStarted = totalStarted,
                inFlightDedupHit = inFlightDedupHit,
                requireItems = false,
            )?.let { return it }
        }
        val awAtFileType = awAtFacade.detectFileType(
            languageId = request.context.languageId,
            documentUri = request.context.documentUri,
        )
        val loadSessionStarted = System.nanoTime()
        val cachedSession = projectService.loadCachedSession(request.context.workspaceRoot)
        val session = cachedSession.session
        val loadSessionMs = elapsedMs(loadSessionStarted)
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
                            replacementRange = when (awAtFileType) {
                                AwAtFileType.ACCESS_WIDENER -> AwContextExtractor.extract(
                                    request.context.bufferText,
                                    request.context.position.line,
                                    request.context.position.character,
                                )?.let { CompletionReplacementRange(it.valueStartOffset, it.valueEndOffset) }
                                AwAtFileType.ACCESS_TRANSFORMER -> AtContextExtractor.extract(
                                    request.context.bufferText,
                                    request.context.position.line,
                                    request.context.position.character,
                                )?.let { CompletionReplacementRange(it.valueStartOffset, it.valueEndOffset) }
                            },
                        ),
                    ),
                    isIncomplete = CompletionItemConverter.isIncomplete(items),
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
                negativeCache.put(
                    requestKey,
                    reason = reason,
                    expiresAtMillis = currentTimeMillis() + negativeTtlMillis(reason),
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
                replacementRange = completion.replacementRange?.let {
                    CompletionReplacementRange(it.startOffset, it.endOffset)
                },
            ),
        )
        val dtoConvertMs = elapsedMs(dtoStarted)
        return McdevResponseEnvelope(
            capabilities = setOf("completion"),
            result = McdevCompletionResponse(
                items = itemDtos,
                isIncomplete = CompletionItemConverter.isIncomplete(completion.items),
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

    private fun bufferOnlyResponse(
        request: McdevCompletionRequest,
        totalStarted: Long,
        inFlightDedupHit: Boolean,
        requireItems: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse>? {
        if (awAtFacade.detectFileType(request.context.languageId, request.context.documentUri) != null) {
            return null
        }
        val options = mixinFacade.toCompletionOptions(
            mixinClassInsert = request.options.mixinClassInsert,
            injectMethodDescriptor = request.options.injectMethodDescriptor,
            preferredAtTarget = request.options.preferredAtTarget,
        )
        val bufferOnly = mixinFacade.tryCompleteBufferOnly(
            source = request.context.bufferText,
            line = request.context.position.line,
            character = request.context.position.character,
            options = options,
            documentUri = request.context.documentUri,
            languageId = request.context.languageId,
        ) ?: return null
        if (requireItems && bufferOnly.result.items.isEmpty()) {
            return null
        }
        return buildBufferOnlyCompletionResponse(
            request = request,
            bufferOnly = bufferOnly,
            options = options,
            totalStarted = totalStarted,
            requestKey = request.cacheKey(),
            inFlightDedupHit = inFlightDedupHit,
        )
    }

    private fun buildBufferOnlyCompletionResponse(
        request: McdevCompletionRequest,
        bufferOnly: BufferOnlyCompletionResult,
        options: MixinCompletionOptions,
        totalStarted: Long,
        requestKey: String,
        inFlightDedupHit: Boolean,
    ): McdevResponseEnvelope<McdevCompletionResponse> {
        val completion = bufferOnly.result
        completion.debug.zeroItemReason?.let { reason ->
            if (completion.items.isEmpty()) {
                negativeCache.put(
                    requestKey,
                    reason = reason,
                    expiresAtMillis = currentTimeMillis() + negativeTtlMillis(reason),
                )
            }
        }
        val bufferTextBytes = request.context.bufferText.toByteArray(Charsets.UTF_8).size
        val payloadBytes = estimatePayloadBytes(request)
        val dtoStarted = System.nanoTime()
        val itemDtos = CompletionItemConverter.toDtos(
            items = completion.items,
            annotationContext = bufferOnly.context,
            source = request.context.bufferText,
            convertContext = CompletionConvertContext(
                source = request.context.bufferText,
                annotationContext = bufferOnly.context,
                classInsertMode = options.classInsertMode,
                preferredAtTarget = options.preferredAtTarget,
                replacementRange = completion.replacementRange?.let {
                    CompletionReplacementRange(it.startOffset, it.endOffset)
                },
            ),
        )
        val dtoConvertMs = elapsedMs(dtoStarted)
        return McdevResponseEnvelope(
            capabilities = setOf("completion"),
            result = McdevCompletionResponse(
                items = itemDtos,
                isIncomplete = CompletionItemConverter.isIncomplete(completion.items),
                debug = completion.debug.toProtocolDebug(
                    totalMs = elapsedMs(totalStarted),
                    payloadBytes = payloadBytes,
                    bufferTextBytes = bufferTextBytes,
                    bufferTextFallbackUsed = request.context.bufferTextFallbackUsed,
                    loadSessionMs = 0,
                    projectSessionCacheHit = false,
                    projectSessionVersion = 0,
                    documentCacheHit = null,
                    documentSnapshotMs = null,
                    documentVersion = request.context.documentVersion,
                    semanticCacheHit = null,
                    astParseMs = null,
                    candidateCacheHit = null,
                    candidateBuildMs = null,
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
        documentCacheHit: Boolean?,
        documentSnapshotMs: Long?,
        documentVersion: Long?,
        semanticCacheHit: Boolean?,
        astParseMs: Long?,
        candidateCacheHit: Boolean?,
        candidateBuildMs: Long?,
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

    private class NegativeCompletionCache(
        private val maxEntries: Int = MAX_NEGATIVE_CACHE_ENTRIES,
        private val currentTimeMillis: () -> Long,
    ) {
        private data class Entry(
            val reason: String,
            val expiresAtMillis: Long,
        )

        private val entries = object : LinkedHashMap<String, Entry>(maxEntries, 0.75f, true) {
            override fun removeEldestEntry(eldest: MutableMap.MutableEntry<String, Entry>?): Boolean =
                size > maxEntries
        }

        @Synchronized
        fun get(key: String): String? {
            val entry = entries[key] ?: return null
            if (currentTimeMillis() > entry.expiresAtMillis) {
                entries.remove(key)
                return null
            }
            return entry.reason
        }

        @Synchronized
        fun put(key: String, reason: String, expiresAtMillis: Long) {
            entries[key] = Entry(reason = reason, expiresAtMillis = expiresAtMillis)
        }

        internal fun sizeForTests(): Int = synchronized(this) { entries.size }

        private companion object {
            const val MAX_NEGATIVE_CACHE_ENTRIES = 256
        }
    }
}
