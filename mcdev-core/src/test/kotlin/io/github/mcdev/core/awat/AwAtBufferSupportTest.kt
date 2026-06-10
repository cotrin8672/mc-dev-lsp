package io.github.mcdev.core.awat

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull

class AwAtBufferSupportTest {
    @Test
    fun detectsAccessWidenerByLanguageId() {
        assertEquals(
            AwAtFileType.ACCESS_WIDENER,
            AwAtBufferSupport.detectFileType("accesswidener", "file:///buffer"),
        )
    }

    @Test
    fun detectsAccessTransformerByLanguageId() {
        assertEquals(
            AwAtFileType.ACCESS_TRANSFORMER,
            AwAtBufferSupport.detectFileType("accesstransformer", "file:///buffer"),
        )
    }

    @Test
    fun detectsAccessWidenerByExtension() {
        assertEquals(
            AwAtFileType.ACCESS_WIDENER,
            AwAtBufferSupport.detectFileType("plaintext", "file:///src/mod.accesswidener"),
        )
        assertEquals(
            AwAtFileType.ACCESS_WIDENER,
            AwAtBufferSupport.detectFileType("java", "file:///src/mod.aw"),
        )
    }

    @Test
    fun detectsAccessTransformerByExtension() {
        assertEquals(
            AwAtFileType.ACCESS_TRANSFORMER,
            AwAtBufferSupport.detectFileType("plaintext", "file:///META-INF/mod_at.cfg"),
        )
        assertEquals(
            AwAtFileType.ACCESS_TRANSFORMER,
            AwAtBufferSupport.detectFileType("java", "file:///accesstransformer.cfg"),
        )
    }

    @Test
    fun returnsNullForJavaBuffers() {
        assertNull(AwAtBufferSupport.detectFileType("java", "file:///Example.java"))
    }
}
