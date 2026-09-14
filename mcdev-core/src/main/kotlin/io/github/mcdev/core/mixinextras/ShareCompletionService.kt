package io.github.mcdev.core.mixinextras

import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.mixin.AnnotationContext
import io.github.mcdev.core.mixin.AnnotationContextExtractor
import io.github.mcdev.core.mixin.AnnotationSlot
import io.github.mcdev.core.mixin.ClassIndex
import io.github.mcdev.core.mixin.JavaSourceImports
import io.github.mcdev.core.mixin.JavaTypeDescriptorResolver
import io.github.mcdev.core.mixin.MixinAnnotation
import io.github.mcdev.core.mixin.MixinTargetResolver

class ShareCompletionService(
    private val classIndex: ClassIndex,
    private val signatureService: HandlerSignatureService = HandlerSignatureService(classIndex),
) {
    fun complete(
        source: String,
        context: AnnotationContext,
        additionalSources: Sequence<String> = emptySequence(),
    ): List<McCompletionItem> {
        if (context.annotation != MixinAnnotation.SHARE ||
            context.slot != AnnotationSlot.VALUE ||
            context.attributeName == "namespace"
        ) return emptyList()

        AnnotationContextExtractor.resolveMixinClassScope(source, context.annotationStartOffset) ?: return emptyList()
        val sites = HandlerSignatureService.findSugarHandlerAnnotationSites(source)
        val imports = JavaTypeDescriptorResolver.importsFor(source)
        val currentKeys = sites.asSequence().mapNotNull { site ->
            val handler = site.handlerMethod ?: return@mapNotNull null
            val scope = scopeFor(site, source) ?: return@mapNotNull null
            val parameter = HandlerSignatureService.enrichHandlerTypes(handler, classIndex).parameters
                .firstOrNull { it.sugarSpec is HandlerParameterSugarSpec.Share &&
                    contains(source, it.sugarAnnotationRange, context.annotationStartOffset) }
                ?: return@mapNotNull null
            val share = parameter.sugarSpec as HandlerParameterSugarSpec.Share
            val namespace = effectiveNamespace(share, scope.className) ?: return@mapNotNull null
            val refDescriptor = HandlerSignatureService.shareRefValueDescriptor(parameter.typeDescriptor ?: return@mapNotNull null)
                ?: return@mapNotNull null
            val targetMethods = resolveTargetMethods(site, scope, imports)
            targetMethods.map { target -> ShareKey(target.owner, target.name, target.descriptor, namespace, refDescriptor) }
        }.flatten().toSet()
        if (currentKeys.isEmpty()) return emptyList()

        val values = linkedSetOf<String>()
        fun collectValues(candidateSource: String, isCurrentSource: Boolean) {
            val candidateSites = if (isCurrentSource) sites else HandlerSignatureService.findSugarHandlerAnnotationSites(candidateSource)
            val candidateImports = if (isCurrentSource) imports else JavaTypeDescriptorResolver.importsFor(candidateSource)
            for (site in candidateSites) {
                val handler = site.handlerMethod ?: continue
                val scope = scopeFor(site, candidateSource) ?: continue
                val targetMethods = resolveTargetMethods(site, scope, candidateImports)
                if (targetMethods.isEmpty()) continue
                val enriched = HandlerSignatureService.enrichHandlerTypes(handler, classIndex)
                for (parameter in enriched.parameters) {
                    val share = parameter.sugarSpec as? HandlerParameterSugarSpec.Share ?: continue
                    val value = share.value ?: continue
                    if (value.isEmpty()) continue
                    if (isCurrentSource && contains(candidateSource, parameter.sugarAnnotationRange, context.annotationStartOffset)) continue
                    val namespace = effectiveNamespace(share, scope.className) ?: continue
                    val refDescriptor = HandlerSignatureService.shareRefValueDescriptor(parameter.typeDescriptor ?: continue)
                        ?: continue
                    if (targetMethods.any { target ->
                            ShareKey(target.owner, target.name, target.descriptor, namespace, refDescriptor) in currentKeys
                        }
                    ) values += value
                }
            }
        }
        collectValues(source, isCurrentSource = true)
        additionalSources.forEach { collectValues(it, isCurrentSource = false) }
        return values.asSequence()
            .map { value -> value to escapeJavaString(value) }
            .filter { (_, escapedValue) -> escapedValue.startsWith(context.partialValue) }
            .sortedBy { (value, _) -> value }
            .mapIndexed { index, (value, escapedValue) ->
                McCompletionItem(
                    label = value,
                    detail = "@Share",
                    documentation = null,
                    filterText = escapedValue,
                    insertText = escapedValue,
                    kind = McCompletionKind.VALUE,
                    sortKey = "0200_${index.toString().padStart(4, '0')}_$value",
                    metadata = McCompletionMetadata(
                        source = "mixinextras.share",
                        name = value,
                    ),
                )
            }
            .toList()
    }

    private fun escapeJavaString(value: String): String =
        value.replace("\\", "\\\\")
            .replace("\"", "\\\"")
            .replace("\b", "\\b")
            .replace("\u000C", "\\f")
            .replace("\n", "\\n")
            .replace("\r", "\\r")
            .replace("\t", "\\t")

    private fun resolveTargetMethods(
        site: MixinExtrasAnnotationSite,
        scope: AnnotationContextExtractor.MixinClassScope,
        imports: JavaSourceImports,
    ): Set<TargetMethod> {
        val targets = MixinTargetResolver.resolveTargets(scope.rawTargets, classIndex, imports)
        val resolved = signatureService.resolveTargetMethod(targets, site.methodAttribute) ?: return emptySet()
        return targets.flatMap { owner ->
            classIndex.getMethods(owner)
                .filter { it.name == resolved.name && it.descriptor == resolved.descriptor }
                .map { TargetMethod(owner, it.name, it.descriptor) }
        }.toSet()
    }

    private fun scopeFor(
        site: MixinExtrasAnnotationSite,
        source: String,
    ): AnnotationContextExtractor.MixinClassScope? {
        val siteOffset = AnnotationContextExtractor.toOffset(
            source,
            site.annotationRange.start.line,
            site.annotationRange.start.character,
        ) ?: return null
        return AnnotationContextExtractor.resolveMixinClassScope(source, siteOffset)
    }

    private fun effectiveNamespace(share: HandlerParameterSugarSpec.Share, defaultNamespace: String): String? =
        if (share.namespaceSpecified) share.namespace else defaultNamespace.takeIf { it.isNotBlank() }

    private fun contains(source: String, range: McTextRange?, offset: Int): Boolean {
        if (range == null) return false
        val start = AnnotationContextExtractor.toOffset(source, range.start.line, range.start.character) ?: return false
        val end = AnnotationContextExtractor.toOffset(source, range.end.line, range.end.character) ?: return false
        return offset in start..end
    }

    private data class TargetMethod(
        val owner: String,
        val name: String,
        val descriptor: String,
    )

    private data class ShareKey(
        val owner: String,
        val name: String,
        val descriptor: String,
        val namespace: String,
        val refDescriptor: String,
    )
}
