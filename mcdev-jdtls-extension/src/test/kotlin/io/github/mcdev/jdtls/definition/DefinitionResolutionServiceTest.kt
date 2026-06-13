package io.github.mcdev.jdtls.definition

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import io.github.mcdev.protocol.McdevDefinitionResolution
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class DefinitionResolutionServiceTest {
    @TempDir
    lateinit var tempDir: Path

    private val service = FileBasedProjectContextService()

    @Test
    fun prefersSourceBackendOverJdtBackend() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_BASIC, tempDir)
        val context = service.buildProjectContext(tempDir)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val jdtCalled = booleanArrayOf(false)
        val service = DefinitionResolutionService(
            backends = listOf(
                SourceDefinitionBackend(),
                DefinitionBackend { _, _, _ ->
                    jdtCalled[0] = true
                    ResolvedDefinition(
                        target = target,
                        documentUri = "file:///jdt/SimpleTarget.java",
                        range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
                        resolution = McdevDefinitionResolution.JDT,
                    )
                },
            ),
        )
        val resolved = service.resolveAll(
            targets = listOf(target),
            projectContext = context,
            workspaceRootUri = JdtlsFixtureSupport.workspaceUri(tempDir),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.single().resolution)
        assertEquals(false, jdtCalled[0])
    }

    @Test
    fun fallsBackToJdtBackendWhenSourceMissing() {
        val context = projectContext(tempDir)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val service = DefinitionResolutionService(
            backends = listOf(
                SourceDefinitionBackend { SourceIndex.fromEntries(emptyList()) },
                DefinitionBackend { resolvedTarget, _, _ ->
                    ResolvedDefinition(
                        target = resolvedTarget,
                        documentUri = "file:///jdt/SimpleTarget.java",
                        range = McTextRange(McTextPosition(4, 0), McTextPosition(4, 10)),
                        resolution = McdevDefinitionResolution.JDT,
                    )
                },
            ),
        )
        val resolved = service.resolveAll(
            targets = listOf(target),
            projectContext = context,
            workspaceRootUri = JdtlsFixtureSupport.workspaceUri(tempDir),
        )
        assertEquals(McdevDefinitionResolution.JDT, resolved.single().resolution)
        assertTrue(resolved.single().documentUri.contains("jdt"))
    }

    @Test
    fun resolvesLoomMappedSourcesWithoutJdtBackend() {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_LOOM_E2E, tempDir)
        val context = service.buildProjectContext(tempDir)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/target/SimpleTarget",
            ownerFqn = "com.example.target.SimpleTarget",
        )
        val resolved = DefinitionResolutionService(
            backends = listOf(
                SourceDefinitionBackend(),
                JdtDefinitionBackend(bridge = null),
            ),
        ).resolveAll(
            targets = listOf(target),
            projectContext = context,
            workspaceRootUri = JdtlsFixtureSupport.workspaceUri(tempDir),
        )
        assertEquals(McdevDefinitionResolution.SOURCE, resolved.single().resolution)
        assertTrue(resolved.single().documentUri.contains("mapped-sources"))
        assertTrue(resolved.single().documentUri.contains("SimpleTarget.java"))
    }

    @Test
    fun returnsUnresolvedWhenNoBackendMatches() {
        val context = projectContext(tempDir)
        val target = McDefinitionTarget(
            kind = MemberKind.CLASS,
            ownerInternalName = "com/example/missing/Missing",
            ownerFqn = "com.example.missing.Missing",
        )
        val service = DefinitionResolutionService(
            backends = listOf(
                SourceDefinitionBackend { SourceIndex.fromEntries(emptyList()) },
                JdtDefinitionBackend(bridge = null),
            ),
        )
        val resolved = service.resolveAll(
            targets = listOf(target),
            projectContext = context,
            workspaceRootUri = JdtlsFixtureSupport.workspaceUri(tempDir),
        )
        assertEquals(McdevDefinitionResolution.UNRESOLVED, resolved.single().resolution)
        assertTrue(resolved.single().resolutionMessage?.contains("no navigable definition") == true)
    }

    private fun projectContext(root: Path): ProjectContext = service.buildProjectContext(root)
}
