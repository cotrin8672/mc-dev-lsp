package io.github.mcdev.jdtls.transport

import com.google.gson.Gson
import io.github.mcdev.core.mixinextras.OfficialExpressionCancellationChecker
import io.github.mcdev.core.mixinextras.OfficialExpressionCancellationContext
import io.github.mcdev.protocol.McdevError
import io.github.mcdev.protocol.McdevErrorCode
import io.github.mcdev.protocol.McdevResponseEnvelope
import java.io.BufferedInputStream
import java.io.BufferedOutputStream
import java.io.Closeable
import java.io.EOFException
import java.io.IOException
import java.io.InputStream
import java.net.InetAddress
import java.net.InetSocketAddress
import java.net.ServerSocket
import java.net.Socket
import java.net.SocketException
import java.net.SocketTimeoutException
import java.nio.charset.StandardCharsets
import java.security.SecureRandom
import java.util.Base64
import java.util.Collections
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import java.util.concurrent.Future
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import java.util.concurrent.atomic.AtomicBoolean

/**
 * A small same-process bridge for project-aware completion.
 *
 * JDT LS puts custom executeCommand requests behind its global index barrier. This bridge calls the
 * same dispatcher from a loopback-only authenticated socket, so the completion path keeps its full
 * project-aware semantics without waiting for that unrelated barrier.
 */
class McdevCompletionTransport(
    private val execute: (Map<String, Any?>) -> Map<String, Any?>,
    private val requestTimeoutMillis: Long = DEFAULT_TIMEOUT_MILLIS,
    private val maxBodyBytes: Int = DEFAULT_MAX_BODY_BYTES,
    private val notifyEndpoint: (Endpoint) -> Boolean = { false },
    private val onFailure: (Throwable) -> Unit = {},
) : Closeable {
    init {
        require(requestTimeoutMillis > 0) { "requestTimeoutMillis must be positive" }
        require(maxBodyBytes > 0) { "maxBodyBytes must be positive" }
    }

    private val gson = Gson()
    private val closed = AtomicBoolean(false)
    private val server = ServerSocket()
    private val acceptExecutor = Executors.newSingleThreadExecutor(daemonThreadFactory("mcdev-transport-accept"))
    private val connectionExecutor = Executors.newCachedThreadPool(daemonThreadFactory("mcdev-transport-connection"))
    private val computeExecutor = Executors.newCachedThreadPool(daemonThreadFactory("mcdev-transport-compute"))
    private val notificationExecutor = Executors.newSingleThreadScheduledExecutor(daemonThreadFactory("mcdev-transport-notify"))
    private val activeSockets = Collections.newSetFromMap(ConcurrentHashMap<Socket, Boolean>())
    private val token = randomToken()
    private var endpoint: Endpoint? = null
    private var notificationTask: ScheduledFuture<*>? = null
    private var notificationStartedAtNanos: Long = 0

    @Synchronized
    fun start(): Endpoint {
        endpoint?.let { return it }
        server.reuseAddress = true
        server.bind(InetSocketAddress(InetAddress.getByName(LOOPBACK_HOST), 0))
        val started = Endpoint(
            host = LOOPBACK_HOST,
            port = server.localPort,
            token = token,
            timeoutMs = requestTimeoutMillis,
        )
        endpoint = started
        acceptExecutor.execute(::acceptLoop)
        notificationStartedAtNanos = System.nanoTime()
        notificationTask = notificationExecutor.scheduleAtFixedRate({
            if (closed.get()) return@scheduleAtFixedRate
            if (System.nanoTime() - notificationStartedAtNanos > NOTIFICATION_DEADLINE_NANOS) {
                notificationTask?.cancel(false)
                return@scheduleAtFixedRate
            }
            try {
                if (notifyEndpoint(started)) notificationTask?.cancel(false)
            } catch (error: Throwable) {
                onFailure(error)
            }
        }, 0, NOTIFICATION_RETRY_MILLIS, TimeUnit.MILLISECONDS)
        return started
    }

    fun endpoint(): Endpoint? = endpoint

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        runClose(server)
        activeSockets.forEach(::runClose)
        notificationTask?.cancel(false)
        notificationExecutor.shutdownNow()
        acceptExecutor.shutdownNow()
        connectionExecutor.shutdownNow()
        computeExecutor.shutdownNow()
    }

    private fun acceptLoop() {
        while (!closed.get()) {
            try {
                val socket = server.accept()
                activeSockets.add(socket)
                connectionExecutor.execute { handleConnection(socket) }
            } catch (error: SocketException) {
                if (!closed.get()) onFailure(error)
                return
            } catch (error: IOException) {
                if (!closed.get()) onFailure(error)
            }
        }
    }

    private fun handleConnection(socket: Socket) {
        var disconnectWatcher: Future<*>? = null
        val cancelled = AtomicBoolean(false)
        try {
            socket.soTimeout = requestTimeoutMillis.coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
            val request = readRequest(socket)
            val computation = computeExecutor.submit<Map<String, Any?>> {
                val checker = OfficialExpressionCancellationChecker {
                    if (cancelled.get() || Thread.currentThread().isInterrupted) {
                        throw java.util.concurrent.CancellationException("mcdev completion request cancelled")
                    }
                }
                OfficialExpressionCancellationContext.withChecker(checker) {
                    execute(request.payload)
                }
            }
            socket.soTimeout = DISCONNECT_POLL_MILLIS
            disconnectWatcher = connectionExecutor.submit {
                watchClientDisconnect(request.input, cancelled, computation)
            }
            val response = awaitResponse(computation, cancelled)
            writeResponse(socket, 200, gson.toJson(response))
        } catch (error: TimeoutException) {
            cancelled.set(true)
            writeResponse(socket, 504, gson.toJson(errorEnvelope("mcdev completion request timed out")))
        } catch (error: java.util.concurrent.CancellationException) {
            if (!cancelled.get()) {
                writeResponse(socket, 499, gson.toJson(errorEnvelope("mcdev completion request cancelled")))
            }
        } catch (error: IllegalArgumentException) {
            writeResponse(socket, 400, gson.toJson(errorEnvelope(error.message ?: "invalid mcdev completion request")))
        } catch (error: IOException) {
            if (!closed.get()) onFailure(error)
        } catch (error: Throwable) {
            if (!closed.get()) {
                onFailure(error)
                writeResponse(socket, 500, gson.toJson(errorEnvelope(error.message ?: error.javaClass.name)))
            }
        } finally {
            cancelled.set(true)
            disconnectWatcher?.cancel(true)
            activeSockets.remove(socket)
            runClose(socket)
        }
    }

    private fun awaitResponse(
        computation: Future<Map<String, Any?>>,
        cancelled: AtomicBoolean,
    ): Map<String, Any?> {
        val deadline = System.nanoTime() + TimeUnit.MILLISECONDS.toNanos(requestTimeoutMillis)
        while (true) {
            if (cancelled.get()) {
                computation.cancel(true)
                throw java.util.concurrent.CancellationException("mcdev completion request cancelled")
            }
            val remainingNanos = deadline - System.nanoTime()
            if (remainingNanos <= 0) {
                computation.cancel(true)
                throw TimeoutException()
            }
            try {
                return computation.get(
                    minOf(remainingNanos, TimeUnit.MILLISECONDS.toNanos(DISCONNECT_POLL_MILLIS.toLong())),
                    TimeUnit.NANOSECONDS,
                )
            } catch (_: TimeoutException) {
                // Recheck cancellation and the absolute deadline on the next pass.
            }
        }
    }

    private fun watchClientDisconnect(
        input: InputStream,
        cancelled: AtomicBoolean,
        computation: Future<*>,
    ) {
        while (!closed.get() && !cancelled.get() && !Thread.currentThread().isInterrupted) {
            try {
                if (input.read() < 0) {
                    cancelled.set(true)
                    computation.cancel(true)
                    return
                }
            } catch (_: SocketTimeoutException) {
                // The short read timeout is the disconnect poll interval.
            } catch (error: IOException) {
                if (!closed.get() && !cancelled.get()) onFailure(error)
                cancelled.set(true)
                computation.cancel(true)
                return
            }
        }
    }

    private fun readRequest(socket: Socket): ParsedRequest {
        val input = BufferedInputStream(socket.getInputStream())
        val header = readHeader(input)
        val lines = header.split("\r\n")
        val requestLine = lines.firstOrNull().orEmpty().split(' ')
        require(requestLine.size == 3 && requestLine[0] == "POST" && requestLine[1] == "/completion") {
            "mcdev completion transport accepts POST /completion"
        }
        val headers = lines.drop(1).mapNotNull { line ->
            val separator = line.indexOf(':')
            if (separator <= 0) null else line.substring(0, separator).trim().lowercase() to line.substring(separator + 1).trim()
        }.toMap()
        require(headers["authorization"] == "Bearer $token") { "mcdev completion transport authentication failed" }
        val contentLength = headers["content-length"]?.toIntOrNull()
            ?: throw IllegalArgumentException("content-length is required")
        require(contentLength in 1..maxBodyBytes) { "mcdev completion request body is too large" }
        val body = ByteArray(contentLength)
        var offset = 0
        while (offset < body.size) {
            val read = input.read(body, offset, body.size - offset)
            if (read < 0) throw EOFException("incomplete mcdev completion request body")
            offset += read
        }
        @Suppress("UNCHECKED_CAST")
        val payload = try {
            gson.fromJson(String(body, StandardCharsets.UTF_8), Map::class.java) as? Map<String, Any?>
                ?: throw IllegalArgumentException("mcdev completion request must be a JSON object")
        } catch (error: RuntimeException) {
            throw IllegalArgumentException("mcdev completion request is not valid JSON", error)
        }
        return ParsedRequest(payload, input)
    }

    private fun readHeader(input: BufferedInputStream): String {
        val bytes = ByteArrayOutputStreamLimit(MAX_HEADER_BYTES)
        var previous = 0
        var current: Int
        while (true) {
            current = input.read()
            if (current < 0) throw EOFException("incomplete mcdev completion request")
            bytes.write(current)
            if (previous == '\r'.code && current == '\n'.code && bytes.endsWith("\r\n\r\n")) {
                return bytes.toByteArray().toString(StandardCharsets.ISO_8859_1).removeSuffix("\r\n\r\n")
            }
            previous = current
            if (bytes.size > MAX_HEADER_BYTES) throw IllegalArgumentException("mcdev completion request headers are too large")
        }
    }

    private fun writeResponse(socket: Socket, status: Int, body: String) {
        val bytes = body.toByteArray(StandardCharsets.UTF_8)
        val output = BufferedOutputStream(socket.getOutputStream())
        output.write("HTTP/1.1 $status ${statusText(status)}\r\n".toByteArray(StandardCharsets.US_ASCII))
        output.write("Content-Type: application/json\r\n".toByteArray(StandardCharsets.US_ASCII))
        output.write("Content-Length: ${bytes.size}\r\nConnection: close\r\n\r\n".toByteArray(StandardCharsets.US_ASCII))
        output.write(bytes)
        output.flush()
    }

    private fun errorEnvelope(message: String): McdevResponseEnvelope<Nothing> =
        McdevResponseEnvelope(
            error = McdevError(
                code = McdevErrorCode.INTERNAL_ERROR,
                message = message,
            ),
        )

    private fun runClose(closeable: Closeable) {
        try {
            closeable.close()
        } catch (error: IOException) {
            if (!closed.get()) onFailure(error)
        }
    }

    data class Endpoint(
        val host: String,
        val port: Int,
        val token: String,
        val timeoutMs: Long,
        val protocolVersion: Int = 1,
    ) {
        fun asMap(): Map<String, Any> = mapOf(
            "host" to host,
            "port" to port,
            "token" to token,
            "timeoutMs" to timeoutMs,
            "protocolVersion" to protocolVersion,
        )
    }

    private data class ParsedRequest(
        val payload: Map<String, Any?>,
        val input: InputStream,
    )

    private class ByteArrayOutputStreamLimit(private val limit: Int) {
        private val bytes = java.io.ByteArrayOutputStream()
        val size: Int get() = bytes.size()
        fun write(value: Int) {
            if (size >= limit) throw IllegalArgumentException("mcdev completion request headers are too large")
            bytes.write(value)
        }
        fun endsWith(suffix: String): Boolean {
            val data = bytes.toByteArray()
            val target = suffix.toByteArray(StandardCharsets.ISO_8859_1)
            return data.size >= target.size && data.copyOfRange(data.size - target.size, data.size).contentEquals(target)
        }
        fun toByteArray(): ByteArray = bytes.toByteArray()
    }

    private companion object {
        const val LOOPBACK_HOST = "127.0.0.1"
        const val DEFAULT_TIMEOUT_MILLIS = 5_000L
        const val DEFAULT_MAX_BODY_BYTES = 4 * 1024 * 1024
        const val MAX_HEADER_BYTES = 16 * 1024
        const val DISCONNECT_POLL_MILLIS = 250
        const val NOTIFICATION_RETRY_MILLIS = 100L
        const val NOTIFICATION_DEADLINE_NANOS = 30_000_000_000L

        fun randomToken(): String {
            val bytes = ByteArray(32)
            SecureRandom().nextBytes(bytes)
            return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)
        }

        fun daemonThreadFactory(prefix: String): java.util.concurrent.ThreadFactory =
            java.util.concurrent.ThreadFactory { runnable ->
                Thread(runnable, "$prefix-${THREAD_IDS.incrementAndGet()}").apply { isDaemon = true }
            }

        val THREAD_IDS = java.util.concurrent.atomic.AtomicLong()

        fun statusText(status: Int): String = when (status) {
            200 -> "OK"
            400 -> "Bad Request"
            499 -> "Client Closed Request"
            500 -> "Internal Server Error"
            504 -> "Gateway Timeout"
            else -> "Error"
        }
    }
}
