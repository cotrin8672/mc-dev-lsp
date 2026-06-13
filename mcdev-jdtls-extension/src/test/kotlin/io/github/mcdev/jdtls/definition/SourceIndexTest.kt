package io.github.mcdev.jdtls.definition

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.fixtures.FixtureResourceLoader
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDefinitionResolution
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class SourceIndexTest {
    @TempDir
    lateinit var tempDir: Path

    @Test
    fun resolvesClassDeclarationInProjectSource() {
        val simpleTarget = FixtureResourceLoader.loadText(
            "${FixturePaths.FABRIC_BASIC}/src/main/java/com/example/target/SimpleTarget.java",
        )
        val uri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/target/SimpleTarget.java"
        val index = SourceIndex.fromEntries(listOf(SourceScanEntry(uri, simpleTarget)))
        val resolved = index.resolve(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = "com/example/target/SimpleTarget",
                ownerFqn = "com.example.target.SimpleTarget",
            ),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.resolution)
        assertEquals(uri, resolved.documentUri)
        assertEquals(2, resolved.range.start.line)
        assertTrue(resolved.range.start.character >= 0)
    }

    @Test
    fun resolvesFieldDeclarationInProjectSource() {
        val simpleTarget = FixtureResourceLoader.loadText(
            "${FixturePaths.FABRIC_BASIC}/src/main/java/com/example/target/SimpleTarget.java",
        )
        val uri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/target/SimpleTarget.java"
        val index = SourceIndex.fromEntries(listOf(SourceScanEntry(uri, simpleTarget)))
        val resolved = index.resolve(
            McDefinitionTarget(
                kind = MemberKind.FIELD,
                ownerInternalName = "com/example/target/SimpleTarget",
                ownerFqn = "com.example.target.SimpleTarget",
                name = "counter",
                descriptor = "I",
            ),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.resolution)
        assertEquals(uri, resolved.documentUri)
        assertEquals(3, resolved.range.start.line)
    }

    @Test
    fun resolvesMethodDeclarationWithDescriptorDisambiguation() {
        val simpleTarget = FixtureResourceLoader.loadText(
            "${FixturePaths.FABRIC_BASIC}/src/main/java/com/example/target/SimpleTarget.java",
        )
        val uri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/target/SimpleTarget.java"
        val index = SourceIndex.fromEntries(listOf(SourceScanEntry(uri, simpleTarget)))
        val resolved = index.resolve(
            McDefinitionTarget(
                kind = MemberKind.METHOD,
                ownerInternalName = "com/example/target/SimpleTarget",
                ownerFqn = "com.example.target.SimpleTarget",
                name = "draw",
                descriptor = "(Ljava/lang/String;FF)V",
            ),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.resolution)
        assertEquals(5, resolved.range.start.line)
    }

    @Test
    fun resolvesSameArityOverloadByParameterDescriptor() {
        val source = """
            package com.example.target;

            public class OverloadedTarget {
                public void setValue(int value) {
                }

                public void setValue(String value) {
                }
            }
        """.trimIndent()
        val uri = "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/target/OverloadedTarget.java"
        val index = SourceIndex.fromEntries(listOf(SourceScanEntry(uri, source)))
        val resolved = index.resolve(
            McDefinitionTarget(
                kind = MemberKind.METHOD,
                ownerInternalName = "com/example/target/OverloadedTarget",
                ownerFqn = "com.example.target.OverloadedTarget",
                name = "setValue",
                descriptor = "(Ljava/lang/String;)V",
            ),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.resolution)
        assertEquals(6, resolved.range.start.line)
    }

    @Test
    fun returnsUnresolvedWhenSourceFileMissing() {
        val index = SourceIndex.fromEntries(emptyList())
        val resolved = index.resolve(
            McDefinitionTarget(
                kind = MemberKind.CLASS,
                ownerInternalName = "com/example/missing/Missing",
                ownerFqn = "com.example.missing.Missing",
            ),
        )
        assertEquals(McdevDefinitionResolution.UNRESOLVED, resolved.resolution)
        assertTrue(resolved.resolutionMessage?.contains("no project source file") == true)
    }
}
