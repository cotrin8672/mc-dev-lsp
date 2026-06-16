package io.github.mcdev.jdtls.command

import io.github.mcdev.jdtls.handler.McdevCodeActionHandler
import io.github.mcdev.jdtls.handler.McdevCompletionHandler
import io.github.mcdev.jdtls.handler.McdevDefinitionHandler
import io.github.mcdev.jdtls.handler.McdevDiagnosticsHandler
import io.github.mcdev.jdtls.handler.McdevHoverHandler
import io.github.mcdev.jdtls.handler.McdevInfoHandler
import io.github.mcdev.jdtls.handler.McdevProjectContextHandler
import io.github.mcdev.jdtls.handler.McdevReferencesHandler
import io.github.mcdev.jdtls.handler.McdevReindexHandler
import io.github.mcdev.protocol.McdevCommands
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevCommandDispatcher(
    private val completionHandler: McdevCompletionHandler = McdevCompletionHandler(),
    private val diagnosticsHandler: McdevDiagnosticsHandler = McdevDiagnosticsHandler(),
    private val infoHandler: McdevInfoHandler = McdevInfoHandler(),
    private val reindexHandler: McdevReindexHandler = McdevReindexHandler(),
    private val projectContextHandler: McdevProjectContextHandler = McdevProjectContextHandler(),
    private val codeActionHandler: McdevCodeActionHandler = McdevCodeActionHandler(),
    private val definitionHandler: McdevDefinitionHandler = McdevDefinitionHandler(),
    private val referencesHandler: McdevReferencesHandler = McdevReferencesHandler(),
    private val hoverHandler: McdevHoverHandler = McdevHoverHandler(),
) {
    fun execute(command: String, arguments: List<Any?>): McdevResponseEnvelope<*> =
        try {
            when (command) {
                McdevCommands.COMPLETION -> completionHandler.handle(arguments)
                McdevCommands.DIAGNOSTICS -> diagnosticsHandler.handle(arguments)
                McdevCommands.CONTEXT -> diagnosticsHandler.handle(arguments)
                McdevCommands.INFO -> infoHandler.handle(arguments)
                McdevCommands.REINDEX -> reindexHandler.handle(arguments)
                McdevCommands.RELOAD_PROJECT_CONTEXT -> projectContextHandler.reload(arguments)
                McdevCommands.DUMP_CONTEXT -> projectContextHandler.dump(arguments)
                McdevCommands.CODE_ACTION -> codeActionHandler.handle(arguments)
                McdevCommands.DEFINITION -> definitionHandler.handle(arguments)
                McdevCommands.REFERENCES -> referencesHandler.handle(arguments)
                McdevCommands.HOVER -> hoverHandler.handle(arguments)
                else -> unknownCommand(command)
            }
        } catch (error: RuntimeException) {
            McdevResponseEnvelope<Nothing>(
                error = McdevError(
                    code = McdevErrorCode.INTERNAL_ERROR,
                    message = error.message ?: error.javaClass.name,
                ),
            )
        }

    private fun unsupported(command: String): McdevResponseEnvelope<Nothing> =
        McdevResponseEnvelope(
            error = McdevError(
                code = McdevErrorCode.NO_APPLICABLE_CONTEXT,
                message = "$command is registered but not implemented yet",
            ),
        )

    private fun unknownCommand(command: String): McdevResponseEnvelope<Nothing> =
        McdevResponseEnvelope(
            error = McdevError(
                code = McdevErrorCode.PARSE_ERROR,
                message = "unknown mcdev command: $command",
            ),
        )
}
