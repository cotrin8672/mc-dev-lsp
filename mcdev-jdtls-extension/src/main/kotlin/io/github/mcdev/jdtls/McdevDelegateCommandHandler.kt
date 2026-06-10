package io.github.mcdev.jdtls

import io.github.mcdev.jdtls.command.McdevCommandRegistry
import io.github.mcdev.jdtls.command.McdevCommandDispatcher
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import org.eclipse.core.runtime.IProgressMonitor
import org.eclipse.jdt.ls.core.internal.IDelegateCommandHandler

class McdevDelegateCommandHandler : IDelegateCommandHandler {
    private val decoder = ProtocolPayloadDecoder()

    override fun executeCommand(
        commandId: String,
        arguments: List<Any>,
        monitor: IProgressMonitor,
    ): Any {
        if (!McdevCommandRegistry.implementedCommandIds.contains(commandId)) {
            throw UnsupportedOperationException("mcdev command is not registered: $commandId")
        }

        val dispatcher = McdevServices.dispatcher ?: McdevCommandDispatcher().also {
            McdevServices.dispatcher = it
        }

        @Suppress("UNCHECKED_CAST")
        val payloadArguments = arguments as List<Any?>
        val envelope = dispatcher.execute(commandId, payloadArguments)
        return decoder.encodeToMap(envelope)
    }
}
