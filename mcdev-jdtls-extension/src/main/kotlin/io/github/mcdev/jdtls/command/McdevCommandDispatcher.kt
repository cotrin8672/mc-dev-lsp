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
import io.github.mcdev.jdtls.mixin.MixinServiceFacade
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.protocol.McdevCommands
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevResponseEnvelope

class McdevCommandDispatcher(
    private val completionHandler: McdevCompletionHandler,
    private val diagnosticsHandler: McdevDiagnosticsHandler,
    private val infoHandler: McdevInfoHandler,
    private val reindexHandler: McdevReindexHandler,
    private val projectContextHandler: McdevProjectContextHandler,
    private val codeActionHandler: McdevCodeActionHandler,
    private val definitionHandler: McdevDefinitionHandler,
    private val referencesHandler: McdevReferencesHandler,
    private val hoverHandler: McdevHoverHandler,
) {
    constructor() : this(defaultHandlers())

    private constructor(handlers: DefaultHandlers) : this(
        completionHandler = handlers.completion,
        diagnosticsHandler = handlers.diagnostics,
        infoHandler = handlers.info,
        reindexHandler = handlers.reindex,
        projectContextHandler = handlers.projectContext,
        codeActionHandler = handlers.codeAction,
        definitionHandler = handlers.definition,
        referencesHandler = handlers.references,
        hoverHandler = handlers.hover,
    )

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

    private data class DefaultHandlers(
        val completion: McdevCompletionHandler,
        val diagnostics: McdevDiagnosticsHandler,
        val info: McdevInfoHandler,
        val reindex: McdevReindexHandler,
        val projectContext: McdevProjectContextHandler,
        val codeAction: McdevCodeActionHandler,
        val definition: McdevDefinitionHandler,
        val references: McdevReferencesHandler,
        val hover: McdevHoverHandler,
    )

    companion object {
        private fun defaultHandlers(): DefaultHandlers {
            val projectService = FileBasedProjectContextService()
            val mixinFacade = MixinServiceFacade()
            return DefaultHandlers(
                completion = McdevCompletionHandler(projectService = projectService, mixinFacade = mixinFacade),
                diagnostics = McdevDiagnosticsHandler(projectService = projectService, mixinFacade = mixinFacade),
                info = McdevInfoHandler(projectService = projectService),
                reindex = McdevReindexHandler(projectService = projectService),
                projectContext = McdevProjectContextHandler(projectService = projectService),
                codeAction = McdevCodeActionHandler(projectService = projectService, mixinFacade = mixinFacade),
                definition = McdevDefinitionHandler(projectService = projectService, mixinFacade = mixinFacade),
                references = McdevReferencesHandler(projectService = projectService, mixinFacade = mixinFacade),
                hover = McdevHoverHandler(projectService = projectService, mixinFacade = mixinFacade),
            )
        }
    }
}
