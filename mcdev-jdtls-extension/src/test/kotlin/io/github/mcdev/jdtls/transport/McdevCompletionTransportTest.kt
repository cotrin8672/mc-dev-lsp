package io.github.mcdev.jdtls.transport

import java.net.Socket
import java.nio.charset.StandardCharsets
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger
import java.util.concurrent.atomic.AtomicReference
import kotlin.test.Test
import kotlin.test.assertContains
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class McdevCompletionTransportTest {
    @Test
    fun authenticatedRequestsAreHandledConcurrentlyAndReturnJson() {
        val transport = McdevCompletionTransport(
            execute = { payload ->
                Thread.sleep(120)
                mapOf("result" to payload, "metadata" to mapOf("source" to "same-jvm"))
            },
            requestTimeoutMillis = 1_000,
            notifyEndpoint = { true },
        )
        try {
            val endpoint = transport.start()
            val clients = Executors.newFixedThreadPool(2)
            val firstFuture = clients.submit<HttpResponse> { send(endpoint, "{\"request\":1}") }
            val secondFuture = clients.submit<HttpResponse> { send(endpoint, "{\"request\":2}") }
            val first = firstFuture.get()
            val second = secondFuture.get()
            clients.shutdownNow()
            assertEquals(200, first.status)
            assertEquals(200, second.status)
            assertContains(first.body, "same-jvm")
            assertContains(second.body, "request")
            assertTrue(endpoint.host == "127.0.0.1")
            assertTrue(endpoint.token.length >= 43)
        } finally {
            transport.close()
        }
    }

    @Test
    fun requestDeadlineProducesBoundedError() {
        val transport = McdevCompletionTransport(
            execute = {
                Thread.sleep(2_000)
                emptyMap()
            },
            requestTimeoutMillis = 50,
            notifyEndpoint = { true },
        )
        try {
            val response = send(transport.start(), "{\"request\":1}")
            assertEquals(504, response.status)
            assertContains(response.body, "timed out")
        } finally {
            transport.close()
        }
    }

    @Test
    fun rejectsUnauthenticatedRequestBeforeDispatch() {
        val executions = AtomicInteger()
        val transport = McdevCompletionTransport(
            execute = {
                executions.incrementAndGet()
                emptyMap()
            },
            notifyEndpoint = { true },
        )
        try {
            val endpoint = transport.start()
            val response = send(endpoint, "{}", authorization = "Bearer wrong-token")
            assertEquals(400, response.status)
            assertContains(response.body, "authentication failed")
            assertEquals(0, executions.get())
        } finally {
            transport.close()
        }
    }

    @Test
    fun disconnectCancelsInFlightComputation() {
        val started = CountDownLatch(1)
        val interrupted = CountDownLatch(1)
        val transport = McdevCompletionTransport(
            execute = {
                started.countDown()
                try {
                    Thread.sleep(10_000)
                } catch (error: InterruptedException) {
                    interrupted.countDown()
                    throw error
                }
                emptyMap()
            },
            requestTimeoutMillis = 5_000,
            notifyEndpoint = { true },
        )
        try {
            val endpoint = transport.start()
            Socket(endpoint.host, endpoint.port).use { socket ->
                val body = "{}"
                val request = buildString {
                    append("POST /completion HTTP/1.1\r\n")
                    append("Host: ${endpoint.host}\r\n")
                    append("Authorization: Bearer ${endpoint.token}\r\n")
                    append("Content-Length: ${body.length}\r\n")
                    append("Connection: close\r\n\r\n")
                    append(body)
                }
                socket.getOutputStream().apply {
                    write(request.toByteArray(StandardCharsets.UTF_8))
                    flush()
                }
                assertTrue(started.await(2, TimeUnit.SECONDS))
                socket.shutdownOutput()
            }
            assertTrue(interrupted.await(2, TimeUnit.SECONDS))
        } finally {
            transport.close()
        }
    }

    @Test
    fun completedRequestDoesNotReportServerTeardownAsFailure() {
        val failureObserved = CountDownLatch(1)
        val failure = AtomicReference<Throwable?>()
        val transport = McdevCompletionTransport(
            execute = { mapOf("ok" to true) },
            notifyEndpoint = { true },
            onFailure = { error ->
                failure.set(error)
                failureObserved.countDown()
            },
        )
        try {
            val response = send(transport.start(), "{\"request\":1}")

            assertEquals(200, response.status)
            assertFalse(
                failureObserved.await(2, TimeUnit.SECONDS),
                "normal response teardown reported ${failure.get()?.javaClass?.name}: ${failure.get()?.message}",
            )
        } finally {
            transport.close()
        }
    }

    private fun send(
        endpoint: McdevCompletionTransport.Endpoint,
        body: String,
        authorization: String = "Bearer ${endpoint.token}",
        path: String = "/completion",
    ): HttpResponse {
        Socket(endpoint.host, endpoint.port).use { socket ->
            socket.soTimeout = 2_000
            val bytes = body.toByteArray(StandardCharsets.UTF_8)
            val request = buildString {
                append("POST $path HTTP/1.1\r\n")
                append("Host: ${endpoint.host}\r\n")
                append("Authorization: $authorization\r\n")
                append("Content-Length: ${bytes.size}\r\n")
                append("Connection: close\r\n\r\n")
                append(body)
            }
            val output = socket.getOutputStream()
            output.write(request.toByteArray(StandardCharsets.UTF_8))
            output.flush()
            val response = socket.getInputStream().bufferedReader(StandardCharsets.UTF_8).readText()
            val separator = response.indexOf("\r\n\r\n")
            val status = response.substringBefore("\r\n").split(' ')[1].toInt()
            return HttpResponse(status, response.substring(separator + 4))
        }
    }

    private data class HttpResponse(val status: Int, val body: String)
}
