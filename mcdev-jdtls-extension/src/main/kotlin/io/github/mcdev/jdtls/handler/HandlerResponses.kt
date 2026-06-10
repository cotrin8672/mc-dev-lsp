package io.github.mcdev.jdtls.handler

import io.github.mcdev.protocol.McdevResponseEnvelope
import io.github.mcdev.protocol.protocolMismatch

@Suppress("UNCHECKED_CAST")
internal fun <T> typedProtocolMismatch(clientVersion: Int): McdevResponseEnvelope<T> =
    protocolMismatch(clientVersion) as McdevResponseEnvelope<T>
