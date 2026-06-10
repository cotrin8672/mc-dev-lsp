package io.github.mcdev.jdtls

import io.github.mcdev.jdtls.command.McdevCommandDispatcher
import org.osgi.framework.BundleActivator
import org.osgi.framework.BundleContext

class McdevPlugin : BundleActivator {
    override fun start(context: BundleContext) {
        McdevServices.dispatcher = McdevCommandDispatcher()
    }

    override fun stop(context: BundleContext) {
        McdevServices.dispatcher = null
    }
}

object McdevServices {
    @Volatile
    var dispatcher: McdevCommandDispatcher? = null
}
