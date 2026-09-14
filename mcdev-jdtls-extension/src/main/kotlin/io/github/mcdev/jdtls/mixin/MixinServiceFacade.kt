package io.github.mcdev.jdtls.mixin

import io.github.mcdev.core.codeaction.McFix
import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.definition.McReferenceLocation
import io.github.mcdev.core.definition.SourceScanEntry
import io.github.mcdev.core.mixin.InjectMethodDescriptorMode
import io.github.mcdev.core.mixin.MixinClassInsertMode
import io.github.mcdev.core.mixin.MixinCompletionOptions
import io.github.mcdev.core.mixin.MixinClassModel
import io.github.mcdev.core.mixin.MixinCompletionResult
import io.github.mcdev.core.mixin.McdevCompletionDebugInfo
import io.github.mcdev.core.mixin.MixinDefinitionService
import io.github.mcdev.core.mixin.MixinFacadeRequest
import io.github.mcdev.core.mixin.MixinReferenceService
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.AtTargetCandidate
import io.github.mcdev.core.mixin.AtValueCompletionService
import io.github.mcdev.core.mixin.ClassIndexEntry
import io.github.mcdev.core.mixin.FieldIndexEntry
import io.github.mcdev.core.mixin.MethodIndexEntry
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinServiceFacade as CoreMixinServiceFacade
import io.github.mcdev.core.mixinextras.ExpressionCompletionPosition
import io.github.mcdev.core.mixinextras.ExpressionMemberCompletionService
import io.github.mcdev.core.mixinextras.ExpressionSupport
import io.github.mcdev.core.diagnostics.McDiagnostic
import io.github.mcdev.core.mixin.BytecodeIndex
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.project.MixinConfigDiscoveryService
import io.github.mcdev.core.project.MixinConfigRef
import io.github.mcdev.core.project.ProjectContext
import io.github.mcdev.core.project.SourceSetResolver
import io.github.mcdev.jdtls.project.McdevProjectSession
import io.github.mcdev.jdtls.project.UriPathSupport
import java.lang.reflect.InvocationTargetException
import java.net.URI
import java.nio.file.Files
import java.nio.file.Path
import java.util.concurrent.CancellationException

internal data class BufferOnlyCompletionResult(
    val result: MixinCompletionResult,
    val context: AnnotationContext?,
)

private object BufferOnlyNoOpClassIndex : ClassIndex {
    override fun findClasses(prefix: String, limit: Int): List<ClassIndexEntry> = emptyList()

    override fun findClass(internalName: String): ClassIndexEntry? = null

    override fun findClassByFqn(fqn: String): ClassIndexEntry? = null

    override fun getMethods(ownerInternalName: String): List<MethodIndexEntry> = emptyList()

    override fun getFields(ownerInternalName: String): List<FieldIndexEntry> = emptyList()
}

private object BufferOnlyNoOpBytecodeIndex : BytecodeIndex {
    override fun getAtTargetCandidates(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
        atValue: String,
    ): List<AtTargetCandidate> = emptyList()

    override fun getReturnOrdinalCount(
        ownerInternalName: String,
        methodName: String,
        methodDescriptor: String?,
    ): Int = 0
}

class MixinServiceFacade internal constructor(
    private val facadeFactory: ((ClassIndex, BytecodeIndex) -> CoreMixinServiceFacade)? = null,
    private val referenceService: MixinReferenceService = MixinReferenceService(),
    private val semanticModelParser: JdtMixinSemanticModelParser = JdtMixinSemanticModelParser(),
    private val semanticModelProvider: (String, String, JdtParseEnvironment) -> MixinClassModel = { source, documentUri, environment ->
        semanticModelParser.parse(source, documentUri, environment)
    },
    private val javaProjectResolver: (String) -> Any? = { documentUri ->
        semanticModelParser.resolveJavaProject(documentUri)
    },
    private val featureProbe: JdtInjectionPointFeatureProbe = JdtInjectionPointFeatureProbe(),
    private val atCodeValuesProvider: (javaProject: Any?, projectSessionVersion: Long) -> List<String> =
        JdtInjectionPointAtCodeIndex()::getValues,
    private val completionIndexCaches: CompletionIndexCaches = CompletionIndexCaches(),
    private val completeOverride: ((
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
    ) -> List<McCompletionItem>)? = null,
    private val projectSourceQueryFactory: (Any) -> ProjectSourceQuery = { JdtProjectSourceQuery(it) },
) {
    fun complete(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
        documentUri: String = "file:///Mixin.java",
    ): List<McCompletionItem> =
        complete(session, source, line, character, options, documentUri, semanticModel(source, documentUri, session))

    fun complete(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
        documentUri: String,
        semanticModel: MixinClassModel,
        projectSessionVersion: Long = 0,
    ): List<McCompletionItem> =
        completeOverride?.invoke(session, source, line, character, options)
            ?: facade(session, source = source, documentUri = documentUri, projectSessionVersion = projectSessionVersion).complete(
                MixinFacadeRequest(
                    bufferText = source,
                    line = line,
                    character = character,
                    documentUri = documentUri,
                    semanticModel = semanticModel,
                ),
                options,
            )

    fun completeWithDebug(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
        documentUri: String,
        semanticModel: MixinClassModel,
        languageId: String,
        projectSessionVersion: Long = 0,
    ): MixinCompletionResult {
        completionIndexCaches.resetDebug()
        completeOverride?.invoke(session, source, line, character, options)?.let { items ->
            return MixinCompletionResult(
                items = items,
                debug = McdevCompletionDebugInfo(
                    command = "mcdev.completion",
                    documentUri = documentUri,
                    languageId = languageId,
                    parseSource = semanticModel.parseSource,
                    parseConfidence = semanticModel.confidence,
                    usedCompilationUnit = semanticModel.debugInfo.usedCompilationUnit,
                    usedJavaProject = semanticModel.debugInfo.usedJavaProject,
                    bindingResolvedCount = semanticModel.debugInfo.bindingResolvedCount,
                    bindingFailedCount = semanticModel.debugInfo.bindingFailedCount,
                    fallbackReason = semanticModel.debugInfo.fallbackReason,
                    semanticContextFound = false,
                    fallbackAnnotationContextUsed = false,
                    fallbackAnnotationContextReason = null,
                    semanticTargetCount = semanticModel.targets.size,
                    semanticMemberCount = semanticModel.members.size,
                    completionContextKind = "OVERRIDE",
                    owner = semanticModel.targets.firstOrNull()?.internalName,
                    methodName = null,
                    methodDescriptor = null,
                    candidateCountBeforeFilter = items.size,
                    candidateCountAfterFilter = items.size,
                    zeroItemReason = if (items.isEmpty()) "NO_CANDIDATES" else null,
                    warnings = semanticModel.warnings,
                ),
            )
        }
        return facade(
            session,
            source = source,
            documentUri = documentUri,
            projectSessionVersion = projectSessionVersion,
        ).completeWithDebug(
            MixinFacadeRequest(
                bufferText = source,
                line = line,
                character = character,
                documentUri = documentUri,
                semanticModel = semanticModel,
            ),
            options,
            command = "mcdev.completion",
            languageId = languageId,
        )
    }

    fun candidateCacheDebug(): CandidateCacheDebug = completionIndexCaches.debug()

    /** A project handle can exist before Gradle has populated its dependency classpath. */
    internal fun hasJavaProject(documentUri: String): Boolean {
        val project = javaProjectResolver(documentUri) ?: return false
        return invokeJavaModel(project, "findType", "org.spongepowered.asm.mixin.Mixin") != null
    }

    internal fun tryCompleteBufferOnly(
        source: String,
        line: Int,
        character: Int,
        options: MixinCompletionOptions,
        documentUri: String,
        languageId: String,
    ): BufferOnlyCompletionResult? {
        if (facadeFactory != null) {
            return null
        }
        val offset = AnnotationContextExtractor.toOffset(source, line, character) ?: return null
        val context = AnnotationContextExtractor.extractAtOffset(source, offset)
        if (context != null && !isBufferOnlyEligible(context)) {
            return null
        }
        val lazyJavaProject = lazy { javaProjectResolver(documentUri) }
        val atValueCompletion = AtValueCompletionService(
            injectionPointSpecifierSupported = {
                featureProbe.injectionPointSpecifierSupported(lazyJavaProject.value, 0L)
            },
            additionalAtValues = {
                atCodeValuesProvider(lazyJavaProject.value, 0L)
            },
        )
        val coreFacade = CoreMixinServiceFacade(
            classIndex = BufferOnlyNoOpClassIndex,
            bytecodeIndex = BufferOnlyNoOpBytecodeIndex,
            atValueCompletion = atValueCompletion,
        )
        val result = coreFacade.completeWithDebug(
            MixinFacadeRequest(
                bufferText = source,
                line = line,
                character = character,
                documentUri = documentUri,
                semanticModel = null,
            ),
            options,
            command = "mcdev.completion",
            languageId = languageId,
        )
        if (context == null && (result.items.isEmpty() || result.replacementRange == null)) {
            return null
        }
        return BufferOnlyCompletionResult(result = result, context = context)
    }

    fun analyzeDiagnostics(
        session: McdevProjectSession,
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
    ): List<McDiagnostic> =
        facade(session).diagnose(buildFacadeRequest(session, projectContext, source, documentUri))

    fun selectedMixinConfig(
        projectContext: ProjectContext,
        source: String,
    ): MixinConfigRef? =
        selectedMixinConfig(projectContext, source, documentUri = null)

    fun selectedMixinConfig(
        projectContext: ProjectContext,
        source: String,
        documentUri: String?,
    ): MixinConfigRef? {
        val mixinClassName = extractMixinClassName(source)
        val mixinPackage = extractPackageName(source)
        return when (val scope = mixinConfigScope(projectContext, documentUri)) {
            MixinConfigScope.ProjectWide ->
                MixinConfigDiscoveryService.selectForMixin(
                    configs = projectContext.mixinConfigs,
                    mixinClassName = mixinClassName,
                    mixinPackage = mixinPackage,
                )
            is MixinConfigScope.SourceSet ->
                MixinConfigDiscoveryService.selectForMixin(
                    configs = scope.sourceSetConfigs,
                    mixinClassName = mixinClassName,
                    mixinPackage = mixinPackage,
                ) ?: MixinConfigDiscoveryService.selectForMixin(
                    configs = scope.sharedConfigs,
                    mixinClassName = mixinClassName,
                    mixinPackage = mixinPackage,
                )
        }
    }

    fun selectedMixinConfigContent(
        projectContext: ProjectContext,
        source: String,
    ): String? =
        selectedMixinConfigContent(projectContext, source, documentUri = null)

    fun selectedMixinConfigContent(
        projectContext: ProjectContext,
        source: String,
        documentUri: String?,
    ): String? =
        selectedMixinConfig(projectContext, source, documentUri)?.path?.let(MixinConfigDiscoveryService::readContent)

    fun codeActions(
        session: McdevProjectSession,
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
        line: Int = 0,
        character: Int = 0,
        diagnosticCode: String? = null,
    ): List<McFix> {
        val request = buildFacadeRequest(
            session = session,
            projectContext = projectContext,
            source = source,
            documentUri = documentUri,
            line = line,
            character = character,
        )
        return facade(session).codeActions(request, diagnosticCode)
    }

    fun definitions(
        session: McdevProjectSession,
        source: String,
        line: Int,
        character: Int,
        documentUri: String = "file:///Mixin.java",
    ): List<McDefinitionTarget> =
        MixinDefinitionService(session.classIndex, session.bytecodeIndex)
            .definitionsAt(
                source = source,
                line = line,
                character = character,
                semanticModel = semanticModel(source, documentUri, session),
                documentUri = documentUri,
            )

    fun references(
        session: McdevProjectSession,
        target: McDefinitionTarget,
        sources: List<SourceScanEntry>,
    ): List<McReferenceLocation> =
        referenceService.findReferences(target, sources)

    fun collectSourceEntries(
        projectContext: ProjectContext,
        currentDocumentUri: String,
        currentBufferText: String,
    ): List<SourceScanEntry> {
        val entries = linkedMapOf<String, String>()
        entries[currentDocumentUri] = currentBufferText
        projectContext.sourceSets.forEach { sourceSet ->
            sourceSet.sourceDirectories.forEach { sourceDir ->
                if (!sourceDir.toFile().isDirectory) return@forEach
                sourceDir.toFile().walkTopDown()
                    .filter { it.isFile && it.extension == "java" }
                    .forEach { file ->
                        val uri = UriPathSupport.pathToUri(file.toPath())
                        if (uri !in entries) {
                            entries[uri] = runCatching { file.readText() }.getOrDefault("")
                        }
                    }
            }
        }
        return entries.map { (uri, text) -> SourceScanEntry(uri, text) }
    }

    fun toCompletionOptions(
        mixinClassInsert: String,
        injectMethodDescriptor: String,
        preferredAtTarget: String = "descriptor",
    ): MixinCompletionOptions =
        MixinCompletionOptions(
            classInsertMode = when (mixinClassInsert.lowercase()) {
                "fqn" -> MixinClassInsertMode.FQN
                else -> MixinClassInsertMode.IMPORT
            },
            injectMethodDescriptorMode = when (injectMethodDescriptor.lowercase()) {
                "always" -> InjectMethodDescriptorMode.ALWAYS
                "never" -> InjectMethodDescriptorMode.NEVER
                else -> InjectMethodDescriptorMode.AUTO
            },
            preferredAtTarget = preferredAtTarget,
        )

    private fun facade(session: McdevProjectSession): CoreMixinServiceFacade =
        createCoreFacade(session.classIndex, session.bytecodeIndex)

    private fun facade(
        session: McdevProjectSession,
        source: String,
        documentUri: String,
        projectSessionVersion: Long = 0,
    ): CoreMixinServiceFacade {
        val javaProject = javaProjectResolver(documentUri)
        val cachedClassIndex = completionIndexCaches.classIndex(
            session.classIndex,
            projectSessionVersion = projectSessionVersion,
        )
        val classIndex = SourceBackedClassIndex(
            delegate = cachedClassIndex,
            sourceQuery = CompositeProjectSourceQuery(
                buildList {
                    add(BufferProjectSourceQuery.fromBuffer(source))
                    javaProject?.let { add(projectSourceQueryFactory(it)) }
                },
            ),
        )
        val bytecodeIndex = completionIndexCaches.bytecodeIndex(
            session.bytecodeIndex,
            projectSessionVersion = projectSessionVersion,
        )
        val atValueCompletion = AtValueCompletionService(
            injectionPointSpecifierSupported = {
                featureProbe.injectionPointSpecifierSupported(javaProject, projectSessionVersion)
            },
            additionalAtValues = {
                atCodeValuesProvider(javaProject, projectSessionVersion)
            },
        )
        val expressionSupport = if (facadeFactory == null) {
            val sharedCache = completionIndexCaches.expressionMemberCompletionCache(
                classIndexDelegate = session.classIndex,
                bytecodeIndexDelegate = session.bytecodeIndex,
                projectSessionVersion = projectSessionVersion,
            )
            ExpressionSupport(
                ExpressionMemberCompletionService(
                    classIndex = classIndex,
                    bytecodeIndex = bytecodeIndex,
                    reachableMembersCache = sharedCache,
                ),
                classIndex,
            )
        } else {
            null
        }
        return createCoreFacade(
            classIndex = classIndex,
            bytecodeIndex = bytecodeIndex,
            atValueCompletion = atValueCompletion,
            expressionSupport = expressionSupport,
            shareSources = {
                collectRegisteredMixinSources(
                    projectContext = session.context,
                    source = source,
                    documentUri = documentUri,
                    javaProject = javaProject,
                )
            },
        )
    }

    private fun createCoreFacade(
        classIndex: ClassIndex,
        bytecodeIndex: BytecodeIndex,
        atValueCompletion: AtValueCompletionService = AtValueCompletionService(),
        expressionSupport: ExpressionSupport? = null,
        shareSources: () -> Sequence<String> = { emptySequence() },
    ): CoreMixinServiceFacade =
        facadeFactory?.invoke(classIndex, bytecodeIndex)
            ?: CoreMixinServiceFacade(
                classIndex = classIndex,
                bytecodeIndex = bytecodeIndex,
                atValueCompletion = atValueCompletion,
                shareSources = shareSources,
                expressionSupport = expressionSupport ?: ExpressionSupport(
                    ExpressionMemberCompletionService(classIndex, bytecodeIndex),
                    classIndex,
                ),
            )

    private fun collectRegisteredMixinSources(
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
        javaProject: Any?,
    ): Sequence<String> {
        val currentClass = extractMixinClassName(source)
        val currentPackage = extractPackageName(source)?.takeIf { it.isNotBlank() }
        val currentFqn = currentClass?.let { className ->
            if (currentPackage == null) className else "$currentPackage.$className"
        }
        val currentPath = runCatching {
            UriPathSupport.uriToPath(documentUri).toAbsolutePath().normalize()
        }.getOrNull()
        val configRefs = when (val scope = mixinConfigScope(projectContext, documentUri)) {
            MixinConfigScope.ProjectWide -> projectContext.mixinConfigs
            is MixinConfigScope.SourceSet -> scope.sourceSetConfigs + scope.sharedConfigs
        }
        return configRefs.asSequence()
            .distinctBy { it.path.toAbsolutePath().normalize() }
            .mapNotNull { MixinConfigDiscoveryService.parse(it.path) }
            .flatMap { config ->
                val configPackage = config.packageName?.trim()?.takeIf { it.isNotEmpty() }
                (config.mixins + config.client + config.server + config.common).asSequence()
                    .map(String::trim)
                    .filter { it.isNotEmpty() }
                    .map { entry -> if (configPackage == null) entry else "$configPackage.$entry" }
            }
            .distinct()
            .mapNotNull { fqn ->
                if (fqn == currentFqn) return@mapNotNull null
                when (val jdtLookup = javaProject?.let { findCompilationUnitSource(it, fqn, currentPath) }) {
                    CompilationUnitLookup.Current -> null
                    is CompilationUnitLookup.Source -> jdtLookup.text.takeIf { it.isNotBlank() }
                    null -> findRegisteredMixinSource(projectContext, fqn, currentPath)
                }
            }
            .distinct()
    }

    private sealed interface CompilationUnitLookup {
        data object Current : CompilationUnitLookup

        data class Source(val text: String) : CompilationUnitLookup
    }

    private fun findCompilationUnitSource(
        javaProject: Any,
        fqn: String,
        currentPath: Path?,
    ): CompilationUnitLookup? {
        val type = invokeJavaModel(javaProject, "findType", fqn) ?: return null
        val compilationUnit = invokeJavaModel(type, "getCompilationUnit") ?: return null
        if (currentPath != null && isCurrentCompilationUnit(compilationUnit, currentPath)) {
            return CompilationUnitLookup.Current
        }
        val source = invokeJavaModel(compilationUnit, "getSource") as? String ?: return null
        return CompilationUnitLookup.Source(source)
    }

    private fun isCurrentCompilationUnit(compilationUnit: Any, currentPath: Path): Boolean {
        val location = invokeJavaModel(compilationUnit, "getResource")
            ?.let { invokeJavaModel(it, "getLocationURI") } ?: return false
        val locationUri = (location as? URI)
            ?: runCatching { URI(location.toString()) }.getOrNull()
            ?: return false
        val normalizedLocation = runCatching { Path.of(locationUri).toAbsolutePath().normalize().toUri() }
            .getOrElse { locationUri.normalize() }
        return normalizedLocation == currentPath.toAbsolutePath().normalize().toUri()
    }

    private fun invokeJavaModel(target: Any, methodName: String, vararg args: Any?): Any? {
        val method = target.javaClass.methods.firstOrNull {
            it.name == methodName && it.parameterCount == args.size
        } ?: return null
        return try {
            method.apply { trySetAccessible() }.invoke(target, *args)
        } catch (throwable: Throwable) {
            unwrapInvocationTarget(throwable).also { cause ->
                if (cause is Error || cause is CancellationException || cause is InterruptedException ||
                    cause.javaClass.name == OPERATION_CANCELED_EXCEPTION
                ) throw cause
            }
            null
        }
    }

    private fun unwrapInvocationTarget(throwable: Throwable): Throwable {
        var cause = throwable
        while (cause is InvocationTargetException) {
            val target = cause.targetException ?: break
            cause = target
        }
        return cause
    }

    private fun findRegisteredMixinSource(
        projectContext: ProjectContext,
        fqn: String,
        currentPath: Path?,
    ): String? {
        val relativePath = fqn.substringBefore('$').replace('.', '/') + ".java"
        return projectContext.sourceSets.asSequence()
            .flatMap { it.sourceDirectories.asSequence() }
            .map { it.resolve(relativePath).toAbsolutePath().normalize() }
            .distinct()
            .filter { it != currentPath }
            .mapNotNull { path ->
                if (!Files.isRegularFile(path)) return@mapNotNull null
                runCatching { Files.readString(path) }.getOrNull()
            }
            .firstOrNull { it.isNotBlank() }
    }

    private companion object {
        const val OPERATION_CANCELED_EXCEPTION = "org.eclipse.core.runtime.OperationCanceledException"
    }

    private fun isBufferOnlyEligible(context: AnnotationContext): Boolean =
        context.slot == AnnotationSlot.ATTRIBUTE ||
            (context.annotation == MixinAnnotation.AT && context.slot == AnnotationSlot.VALUE) ||
            isBufferOnlyEligibleExpressionValue(context)

    private fun isBufferOnlyEligibleExpressionValue(context: AnnotationContext): Boolean {
        if (context.annotation !in setOf(MixinAnnotation.EXPRESSION, MixinAnnotation.EXPRESSIONS)) {
            return false
        }
        if (context.slot != AnnotationSlot.VALUE) {
            return false
        }
        return when (context.expressionCompletionPosition) {
            ExpressionCompletionPosition.STATEMENT_START,
            ExpressionCompletionPosition.VALUE_START,
            ExpressionCompletionPosition.AFTER_METHOD_REFERENCE,
            -> true
            ExpressionCompletionPosition.AFTER_DOT,
            ExpressionCompletionPosition.NONE,
            null,
            -> false
        }
    }

    private fun buildFacadeRequest(
        session: McdevProjectSession,
        projectContext: ProjectContext,
        source: String,
        documentUri: String,
        line: Int = 0,
        character: Int = 0,
    ): MixinFacadeRequest {
        val mixinConfig = selectedMixinConfig(projectContext, source, documentUri)
        val mixinPackage = extractPackageName(source) ?: mixinConfig?.packageName
        val semanticModel = semanticModel(source, documentUri, session)
        return MixinFacadeRequest(
            bufferText = source,
            line = line,
            character = character,
            documentUri = documentUri,
            mixinClassName = extractMixinClassName(source),
            mixinPackage = mixinPackage,
            mixinConfigContent = mixinConfig?.path?.let(MixinConfigDiscoveryService::readContent),
            mixinConfigPath = mixinConfig?.path?.let(UriPathSupport::pathToUri),
            semanticModel = semanticModel,
        )
    }

    private sealed interface MixinConfigScope {
        data object ProjectWide : MixinConfigScope

        data class SourceSet(
            val sourceSetConfigs: List<MixinConfigRef>,
            val sharedConfigs: List<MixinConfigRef>,
        ) : MixinConfigScope
    }

    private fun mixinConfigScope(
        projectContext: ProjectContext,
        documentUri: String?,
    ): MixinConfigScope {
        if (documentUri.isNullOrBlank()) {
            return MixinConfigScope.ProjectWide
        }
        val documentPath = runCatching { UriPathSupport.uriToPath(documentUri) }.getOrNull()
            ?: return MixinConfigScope.ProjectWide
        val sourceSet = SourceSetResolver.containingSourceSet(projectContext.sourceSets, documentPath)
            ?: return MixinConfigScope.ProjectWide
        return MixinConfigScope.SourceSet(
            sourceSetConfigs = MixinConfigDiscoveryService.configsForSourceSet(
                configs = projectContext.mixinConfigs,
                sourceSet = sourceSet,
                allSourceSets = projectContext.sourceSets,
                projectRoot = projectContext.root,
            ),
            sharedConfigs = MixinConfigDiscoveryService.sharedConfigs(
                configs = projectContext.mixinConfigs,
                allSourceSets = projectContext.sourceSets,
                projectRoot = projectContext.root,
            ),
        )
    }

    private fun extractMixinClassName(source: String): String? =
        Regex("""\bclass\s+(\w+)""").find(source)?.groupValues?.get(1)

    private fun extractPackageName(source: String): String? =
        Regex("""^\s*package\s+([\w.]+)\s*;""", RegexOption.MULTILINE).find(source)?.groupValues?.get(1)

    fun semanticModel(source: String, documentUri: String): MixinClassModel =
        semanticModelProvider(source, documentUri, JdtParseEnvironment())

    fun semanticModel(source: String, documentUri: String, session: McdevProjectSession): MixinClassModel =
        semanticModelProvider(
            source,
            documentUri,
            JdtParseEnvironment(
                classpathEntries = session.context.classpath.allEntries.map { it.toString() },
                sourcepathEntries = session.context.sourceSets.flatMap { sourceSet ->
                    sourceSet.sourceDirectories.map { it.toString() }
                },
                unitName = unitNameFor(documentUri, session),
            ),
        )

    private fun unitNameFor(documentUri: String, session: McdevProjectSession): String? {
        val path = runCatching { UriPathSupport.uriToPath(documentUri) }
            .getOrElse {
                runCatching { java.nio.file.Path.of(URI(documentUri)) }.getOrNull()
            } ?: return null
        session.context.sourceSets
            .flatMap { it.sourceDirectories }
            .firstOrNull { sourceDir -> path.startsWith(sourceDir) }
            ?.let { sourceDir ->
                return sourceDir.relativize(path).toString().replace('\\', '/')
            }
        return path.fileName?.toString()
    }
}
