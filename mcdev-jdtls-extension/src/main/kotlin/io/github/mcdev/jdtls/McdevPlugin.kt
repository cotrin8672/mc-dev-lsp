package io.github.mcdev.jdtls

import io.github.mcdev.jdtls.command.McdevCommandDispatcher
import io.github.mcdev.jdtls.protocol.ProtocolPayloadDecoder
import io.github.mcdev.jdtls.transport.McdevCompletionTransport
import org.osgi.framework.BundleActivator
import org.osgi.framework.BundleContext
import java.io.IOException

class McdevPlugin : BundleActivator {
    override fun start(context: BundleContext) {
        McdevServices.dispatcher = McdevCommandDispatcher()
        val decoder = ProtocolPayloadDecoder()
        val transport = McdevCompletionTransport(
            execute = { payload ->
                val dispatcher = McdevServices.dispatcher ?: error("mcdev dispatcher is unavailable")
                decoder.encodeToMap(dispatcher.executeProjectAwareCompletion(listOf(payload)))
            },
            notifyEndpoint = ::sendTransportEndpoint,
            onFailure = ::logTransportFailure,
        )
        try {
            transport.start()
            McdevServices.completionTransport = transport
        } catch (error: IOException) {
            logTransportFailure(error)
            transport.close()
        }
    }

    override fun stop(context: BundleContext) {
        McdevServices.completionTransport?.close()
        McdevServices.completionTransport = null
        McdevServices.dispatcher = null
    }
}

object McdevServices {
    @Volatile
    var dispatcher: McdevCommandDispatcher? = null

    @Volatile
    var completionTransport: McdevCompletionTransport? = null
}

private const val TRANSPORT_READY_COMMAND = "mcdev.transportReady"

private fun sendTransportEndpoint(endpoint: McdevCompletionTransport.Endpoint): Boolean {
    val pluginClass = try {
        Class.forName("org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin")
    } catch (_: ClassNotFoundException) {
        return false
    }
    val plugin = pluginClass.getMethod("getInstance").invoke(null) ?: return false
    val connection = pluginClass.getMethod("getClientConnection").invoke(plugin) ?: return false
    val notification = connection.javaClass.methods.firstOrNull { method ->
        method.name == "sendNotification" && method.parameterCount == 2
    } ?: return false
    val arguments = arrayOf<Any>(TRANSPORT_READY_COMMAND, arrayOf<Any>(endpoint.asMap()))
    notification.invoke(connection, *arguments)
    return true
}

private fun logTransportFailure(error: Throwable) {
    val pluginClass = try {
        Class.forName("org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin")
    } catch (_: ClassNotFoundException) {
        System.err.println("mcdev completion transport failed: ${error.message ?: error.javaClass.name}")
        return
    }
    val logger = pluginClass.methods.firstOrNull { method ->
        method.name == "logException" && method.parameterCount == 1
    }
    if (logger != null) {
        logger.invoke(null, error)
    } else {
        System.err.println("mcdev completion transport failed: ${error.message ?: error.javaClass.name}")
    }
}
