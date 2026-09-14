package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.codeaction.WorkspaceEditFix
import io.github.mcdev.core.mixin.AtTargetKind
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinSemanticModelParser
import io.github.mcdev.core.mixin.MixinServiceFacade as CoreMixinServiceFacade
import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixinextras.ExpressionContext
import io.github.mcdev.core.mixinextras.HandlerSignatureService
import io.github.mcdev.core.mixinextras.MixinExtrasAnnotationSite
import io.github.mcdev.core.mixinextras.MixinExtrasDefinition
import io.github.mcdev.core.mixinextras.MixinExtrasDefinitionIndex
import io.github.mcdev.core.mixinextras.MixinExtrasDiagnosticCodes
import io.github.mcdev.core.mixinextras.MixinExtrasExpression
import io.github.mcdev.core.mixinextras.MixinExtrasExpressionIndex
import io.github.mcdev.core.mixinextras.ResolvedMixinExtrasContext
import io.github.mcdev.core.mixinextras.selectResolvedMixinExtrasContext
import io.github.mcdev.fixtures.FixturePaths
import io.github.mcdev.jdtls.project.FileBasedProjectContextService
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport
import io.github.mcdev.jdtls.support.JdtlsFixtureSupport
import kotlin.io.path.createDirectories
import kotlin.io.path.writeText
import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue
import org.junit.jupiter.api.io.TempDir
import java.nio.file.Path

class MixinServiceFacadeMixinExtrasContextTest {
    @TempDir
    lateinit var tempDir: Path

    private val documentUri by lazy {
        "${JdtlsFixtureSupport.workspaceUri(tempDir)}/src/main/java/com/example/mixin/ExpressionConstantMixin.java"
    }

    @Test
    fun analyzeDiagnosticsForwardSessionAwareResolvedExpressionContext() {
        val context = loadBridgeTestContext()
        val environmentCountBefore = context.observedEnvironments.size

        val diagnostics = context.facade.analyzeDiagnostics(
            session = context.session,
            projectContext = context.session.context,
            source = context.source,
            documentUri = documentUri,
        )
        assertTrue(diagnostics.any { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })

        assertProviderObservedSessionEnvironmentSince(context, environmentCountBefore)

        val envLessDiagnostics = CoreMixinServiceFacade(
            classIndex = context.session.classIndex,
            bytecodeIndex = context.session.bytecodeIndex,
        ).diagnose(
            MixinFacadeRequest(
                bufferText = context.source,
                line = 0,
                character = 0,
                documentUri = documentUri,
                semanticModel = context.facade.semanticModel(context.source, documentUri),
            ),
        )
        assertTrue(envLessDiagnostics.none { it.code == MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE })
    }

    @Test
    fun codeActionsForwardSessionAwareResolvedExpressionContext() {
        val context = loadBridgeTestContext()
        val environmentCountBefore = context.observedEnvironments.size

        val fixes = context.facade.codeActions(
            session = context.session,
            projectContext = context.session.context,
            source = context.source,
            documentUri = documentUri,
            diagnosticCode = MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )

        val fix = fixes.filterIsInstance<WorkspaceEditFix>()
            .single { it.kind == "quickfix.mixinextras.fixHandlerSignature" }
        assertTrue(fix.edits.first().newText.contains("int mcdev${'$'}handler(int original)"))
        assertFalse(fix.edits.first().newText.contains("float mcdev${'$'}handler(float original)"))

        assertProviderObservedSessionEnvironmentSince(context, environmentCountBefore)

        val envLessFixes = CoreMixinServiceFacade(
            classIndex = context.session.classIndex,
            bytecodeIndex = context.session.bytecodeIndex,
        ).codeActions(
            MixinFacadeRequest(
                bufferText = context.source,
                line = context.site.handlerMethod!!.range.start.line,
                character = context.site.handlerMethod!!.range.start.character,
                documentUri = documentUri,
                semanticModel = context.facade.semanticModel(context.source, documentUri),
            ),
            MixinExtrasDiagnosticCodes.WRONG_RETURN_TYPE,
        )
        val envLessFix = envLessFixes.filterIsInstance<WorkspaceEditFix>()
            .singleOrNull { it.kind == "quickfix.mixinextras.fixHandlerSignature" }
        assertTrue(
            envLessFix == null ||
                envLessFix.edits.first().newText.contains("float mcdev${'$'}handler(float original)"),
        )
        assertTrue(
            envLessFix == null ||
                !envLessFix.edits.first().newText.contains("int mcdev${'$'}handler(int original)"),
        )
    }

    private data class BridgeTestContext(
        val facade: MixinServiceFacade,
        val session: McdevProjectSession,
        val source: String,
        val site: MixinExtrasAnnotationSite,
        val observedEnvironments: MutableList<JdtParseEnvironment>,
    )

    private fun loadBridgeTestContext(): BridgeTestContext {
        JdtlsFixtureSupport.copyFixture(FixturePaths.FABRIC_MIXINEXTRAS, tempDir)
        JdtlsFixtureSupport.installClasspathClasses(tempDir)
        val source = expressionMixinSource()
        installMixinSource(source)
        val session = FileBasedProjectContextService().loadSession(JdtlsFixtureSupport.workspaceUri(tempDir))
        assertInstalledBytecodeIndexesTextLength(session)
        val site = HandlerSignatureService.findAnnotationSites(source).single()
        val expectedClasspath = session.context.classpath.allEntries.map { it.toString() }
        val expectedSourcepath = session.context.sourceSets.flatMap { sourceSet ->
            sourceSet.sourceDirectories.map { it.toString() }
        }
        val expectedUnitName = expectedUnitName(documentUri, session)
        val observedEnvironments = mutableListOf<JdtParseEnvironment>()
        val facade = MixinServiceFacade(
            semanticModelProvider = { modelSource, modelDocumentUri, environment ->
                observedEnvironments += environment
                val baseModel = MixinSemanticModelParser.parse(modelSource, modelDocumentUri)
                if (environmentContainsSessionPaths(environment, expectedClasspath, expectedSourcepath)) {
                    authoritativeResolvedModel(baseModel, site)
                } else {
                    baseModel
                }
            },
        )

        val envLessModel = facade.semanticModel(source, documentUri)
        assertNull(
            authoritativeExpressionValues(envLessModel, site),
            "environment-less semantic model must not publish authoritative expression constants",
        )

        val sessionAwareModel = facade.semanticModel(source, documentUri, session)
        assertEquals(
            listOf(MAIN_EXPR_VALUE),
            authoritativeExpressionValues(sessionAwareModel, site),
            "session-aware semantic model must resolve MAIN_EXPR to the authoritative capture expression",
        )

        val authoritativeContext = authoritativeExpressionContext()
        val expectedSignature = HandlerSignatureService(session.classIndex, session.bytecodeIndex)
            .expectedSignature(
                source = source,
                site = site,
                mixinTargets = listOf("com/example/target/SimpleTarget"),
                resolvedContext = authoritativeContext,
            )
        assertNotNull(expectedSignature, "authoritative expression context must yield an expected handler signature")
        assertEquals("I", expectedSignature.returnTypeDescriptor, "lengthCall() must infer int return type")
        assertEquals("I", expectedSignature.parameters.single().typeDescriptor, "lengthCall() must infer int original type")

        return BridgeTestContext(
            facade = facade,
            session = session,
            source = source,
            site = site,
            observedEnvironments = observedEnvironments,
        )
    }

    private fun assertProviderObservedSessionEnvironmentSince(
        context: BridgeTestContext,
        sinceIndex: Int,
    ) {
        val expectedClasspath = context.session.context.classpath.allEntries.map { it.toString() }
        val expectedSourcepath = context.session.context.sourceSets.flatMap { sourceSet ->
            sourceSet.sourceDirectories.map { it.toString() }
        }
        val expectedUnitName = expectedUnitName(documentUri, context.session)
        assertNotNull(expectedUnitName, "session source root must yield a compilation unit name")

        val sutEnvironments = context.observedEnvironments.drop(sinceIndex)
        assertTrue(sutEnvironments.isNotEmpty(), "SUT must invoke semanticModelProvider with session environment")
        val sessionAwareEnvironment = sutEnvironments.first()
        assertEquals(expectedClasspath, sessionAwareEnvironment.classpathEntries)
        assertEquals(expectedSourcepath, sessionAwareEnvironment.sourcepathEntries)
        assertEquals(expectedUnitName, sessionAwareEnvironment.unitName)
    }

    private fun environmentContainsSessionPaths(
        environment: JdtParseEnvironment,
        expectedClasspath: List<String>,
        expectedSourcepath: List<String>,
    ): Boolean =
        environment.classpathEntries.containsAll(expectedClasspath) &&
            environment.sourcepathEntries.containsAll(expectedSourcepath) &&
            expectedClasspath.isNotEmpty()

    private fun authoritativeExpressionContext(): ExpressionContext =
        ExpressionContext(
            expressionIndex = MixinExtrasExpressionIndex(
                expressions = listOf(
                    MixinExtrasExpression(id = "main", values = listOf(MAIN_EXPR_VALUE)),
                ),
            ),
            definitionIndex = MixinExtrasDefinitionIndex(
                definitions = listOf(
                    MixinExtrasDefinition(
                        id = "lengthCall",
                        rawMethodReferences = listOf("Ljava/lang/String;length()I"),
                    ),
                ),
            ),
        )

    private fun authoritativeResolvedModel(
        baseModel: MixinClassModel,
        site: MixinExtrasAnnotationSite,
    ): MixinClassModel =
        baseModel.copy(
            resolvedMixinExtrasContexts = listOf(
                ResolvedMixinExtrasContext(
                    handlerRange = site.handlerMethod?.range ?: site.annotationRange,
                    context = authoritativeExpressionContext(),
                ),
            ),
        )

    private fun installMixinSource(source: String) {
        val mixinPath = UriPathSupport.uriToPath(documentUri)
        mixinPath.parent.createDirectories()
        mixinPath.writeText(source)
    }

    private fun assertInstalledBytecodeIndexesTextLength(session: McdevProjectSession) {
        val candidates = session.bytecodeIndex.getAtTargetCandidates(
            ownerInternalName = "com/example/target/SimpleTarget",
            methodName = "draw",
            methodDescriptor = "(Ljava/lang/String;FF)V",
            atValue = "INVOKE",
        )
        assertTrue(
            candidates.any { candidate ->
                candidate.kind == AtTargetKind.INVOKE &&
                    candidate.owner == "java/lang/String" &&
                    candidate.name == "length" &&
                    candidate.descriptor == "()I"
            },
            "installed SimpleTarget.draw(String,float,float) bytecode must expose text.length()",
        )
    }

    private fun authoritativeExpressionValues(
        model: MixinClassModel,
        site: MixinExtrasAnnotationSite,
    ): List<String>? {
        val resolved = selectResolvedMixinExtrasContext(site, model.resolvedMixinExtrasContexts) ?: return null
        return resolved.context.expressionValuesForAtId(site.atId).takeIf { it.isNotEmpty() }
    }

    private fun expectedUnitName(documentUri: String, session: McdevProjectSession): String? {
        val path = runCatching { UriPathSupport.uriToPath(documentUri) }.getOrNull() ?: return null
        session.context.sourceSets
            .flatMap { it.sourceDirectories }
            .firstOrNull { sourceDir -> path.startsWith(sourceDir) }
            ?.let { sourceDir ->
                return sourceDir.relativize(path).toString().replace('\\', '/')
            }
        return path.fileName?.toString()
    }

    private fun expressionMixinSource(): String = """
        package com.example.mixin;
        import com.example.target.SimpleTarget;
        import com.llamalad7.mixinextras.expression.Expression;
        import com.llamalad7.mixinextras.injector.ModifyExpressionValue;
        import org.spongepowered.asm.mixin.Mixin;
        import org.spongepowered.asm.mixin.injection.At;
        @Mixin(SimpleTarget.class)
        public abstract class ExpressionConstantMixin {
            private static final String MAIN_EXPR = "$MAIN_EXPR_VALUE";
            @Expression(id = "main", value = MAIN_EXPR)
            @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(id = "main", value = "MIXINEXTRAS:EXPRESSION"))
            private float mcdev${"$"}handler(float original) { return original; }
        }
    """.trimIndent()

    private companion object {
        private const val MAIN_EXPR_VALUE = "@(?.lengthCall())"
    }
}
