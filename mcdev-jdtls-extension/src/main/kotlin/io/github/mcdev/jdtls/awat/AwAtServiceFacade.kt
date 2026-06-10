package io.github.mcdev.jdtls.awat

import io.github.mcdev.core.awat.AwAtBufferSupport
import io.github.mcdev.core.awat.AwAtFacadeRequest
import io.github.mcdev.core.awat.AwAtFileType
import io.github.mcdev.core.awat.AwAtServiceFacade as CoreAwAtServiceFacade
import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.jdtls.project.McdevProjectSession

class AwAtServiceFacade(
    private val facadeFactory: (McdevProjectSession) -> CoreAwAtServiceFacade = { session ->
        CoreAwAtServiceFacade(
            classIndex = session.classIndex,
            mappingContext = session.context.mappings,
        )
    },
    private val completeOverride: ((
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        fileType: AwAtFileType,
    ) -> List<McCompletionItem>)? = null,
) {
    fun isAwAtBuffer(languageId: String, documentUri: String): Boolean =
        AwAtBufferSupport.detectFileType(languageId, documentUri) != null

    fun detectFileType(languageId: String, documentUri: String): AwAtFileType? =
        AwAtBufferSupport.detectFileType(languageId, documentUri)

    fun complete(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        fileType: AwAtFileType,
        documentUri: String = defaultDocumentUri(fileType),
    ): List<McCompletionItem> =
        completeOverride?.invoke(session, source, line, character, fileType)
            ?: facade(session).complete(
                AwAtFacadeRequest(
                    bufferText = source,
                    line = line,
                    character = character,
                    documentUri = documentUri,
                    fileType = fileType,
                ),
            )

    fun analyzeDiagnostics(
        session: McdevProjectSession,
        source: String,
        documentUri: String,
        fileType: AwAtFileType,
    ): List<McDiagnostic> =
        facade(session).diagnose(
            AwAtFacadeRequest(
                bufferText = source,
                line = 0,
                character = 0,
                documentUri = documentUri,
                fileType = fileType,
            ),
        )

    fun codeActions(
        session: McdevProjectSession,
        source: String,
        documentUri: String,
        fileType: AwAtFileType,
        diagnosticCode: String? = null,
    ): List<McFix> =
        facade(session).codeActions(
            AwAtFacadeRequest(
                bufferText = source,
                line = 0,
                character = 0,
                documentUri = documentUri,
                fileType = fileType,
            ),
            diagnosticCode,
        )

    private fun facade(session: McdevProjectSession): CoreAwAtServiceFacade = facadeFactory(session)

    private fun defaultDocumentUri(fileType: AwAtFileType): String =
        when (fileType) {
            AwAtFileType.ACCESS_WIDENER -> "file:///mod.accesswidener"
            AwAtFileType.ACCESS_TRANSFORMER -> "file:///mod_at.cfg"
        }
}
