package io.github.mcdev.core.mixin

import io.github.mcdev.core.completion.McCompletionInsertTextFormat
import io.github.mcdev.core.completion.McCompletionItem
import io.github.mcdev.core.completion.McCompletionKind
import io.github.mcdev.core.completion.McCompletionMetadata

private const val WRAP_WITH_CONDITION_V1_FQN = "com.llamalad7.mixinextras.injector.WrapWithCondition"

class MixinAttributeCompletionService {
    private data class AttributeSnippet(
        val name: String,
        val preview: String,
        val insertText: String,
    )

    fun complete(context: AnnotationContext): List<McCompletionItem> {
        if (context.slot != AnnotationSlot.ATTRIBUTE) return emptyList()
        val partial = context.partialValue
        return snippetsFor(context.annotation)
            .asSequence()
            .filter { it.name !in context.existingAttributes }
            .filter { isValidAttributeForContext(context, it.name) }
            .filter { it.name.startsWith(partial, ignoreCase = true) }
            .mapIndexed { index, snippet ->
                McCompletionItem(
                    label = snippet.preview,
                    detail = "@${context.annotation.simpleName} attribute",
                    documentation = "Insert the ${snippet.name} attribute and place the cursor in its value.",
                    filterText = snippet.name,
                    insertText = snippet.insertText,
                    kind = McCompletionKind.KEYWORD,
                    sortKey = "0000_${index.toString().padStart(2, '0')}_${snippet.name}",
                    metadata = McCompletionMetadata(
                        source = "mixin.attribute",
                        name = snippet.name,
                    ),
                    insertTextFormat = McCompletionInsertTextFormat.SNIPPET,
                )
            }
            .toList()
    }

    private fun snippetsFor(annotation: MixinAnnotation): List<AttributeSnippet> = when (annotation) {
        MixinAnnotation.MIXIN -> listOf(
            classLiteral("value"),
            quoted("targets"),
            plain("priority"),
            boolean("remap"),
        )
        MixinAnnotation.INJECT -> injectorBase() + listOf(
            boolean("cancellable"),
            enumValue("locals", "LocalCapture"),
            quoted("id"),
        )
        MixinAnnotation.REDIRECT -> injectorBase()
        MixinAnnotation.MODIFY_ARG -> injectorBase() + plain("index")
        MixinAnnotation.MODIFY_ARGS -> injectorBase()
        MixinAnnotation.MODIFY_VARIABLE -> injectorBase() + listOf(
            plain("ordinal"),
            plain("index"),
            boolean("argsOnly"),
            boolean("print"),
        )
        MixinAnnotation.MODIFY_CONSTANT -> methodBase() + listOf(
            annotationValue("constant", "Constant"),
            annotationValue("slice", "Slice"),
        ) + requirementOptions()
        MixinAnnotation.MODIFY_EXPRESSION_VALUE,
        MixinAnnotation.MODIFY_RETURN_VALUE,
        MixinAnnotation.MODIFY_RECEIVER,
        MixinAnnotation.WRAP_WITH_CONDITION,
        -> mixinExtrasInjectorBase()
        MixinAnnotation.WRAP_OPERATION -> wrapOperationBase()
        MixinAnnotation.WRAP_METHOD -> wrapMethodBase()
        MixinAnnotation.AT -> listOf(
            quoted("value"),
            quoted("target"),
            quoted("slice"),
            plain("ordinal"),
            plain("opcode"),
            enumValue("shift", "At.Shift"),
            plain("by"),
            quotedArray("args"),
            boolean("remap"),
            quoted("id"),
        )
        MixinAnnotation.CONSTANT -> listOf(
            boolean("nullValue"),
            plain("intValue"),
            plain("floatValue"),
            plain("longValue"),
            plain("doubleValue"),
            quoted("stringValue"),
            classLiteral("classValue"),
            plain("ordinal"),
            quoted("slice"),
            plain("expandZeroConditions"),
            boolean("log"),
        )
        MixinAnnotation.SLICE -> listOf(
            quoted("id"),
            annotationValue("from", "At"),
            annotationValue("to", "At"),
        )
        MixinAnnotation.ACCESSOR,
        MixinAnnotation.INVOKER,
        -> listOf(quoted("value"), boolean("remap"))
        MixinAnnotation.SHADOW -> listOf(
            quoted("prefix"),
            quotedArray("aliases"),
            boolean("remap"),
        )
        MixinAnnotation.OVERWRITE -> listOf(
            quoted("constraints"),
            boolean("remap"),
        )
        MixinAnnotation.LOCAL -> listOf(
            boolean("print"),
            plain("ordinal"),
            plain("index"),
            quotedArray("name"),
            boolean("argsOnly"),
            classLiteral("type"),
        )
        MixinAnnotation.SHARE -> listOf(
            quoted("value"),
            quoted("namespace"),
        )
        MixinAnnotation.DEFINITION -> listOf(
            quoted("id"),
            quotedArray("method"),
            quotedArray("field"),
            classArray("type"),
            localArray(),
            boolean("remap"),
        )
        MixinAnnotation.EXPRESSION -> listOf(
            quotedArray("value"),
            quoted("id"),
        )
        MixinAnnotation.DEFINITIONS -> listOf(definitionsValue())
        MixinAnnotation.EXPRESSIONS -> listOf(expressionsValue())
    }

    private fun injectorBase(): List<AttributeSnippet> = methodBase() + listOf(
        at(),
        annotationValue("slice", "Slice"),
    ) + requirementOptions()

    private fun mixinExtrasInjectorBase(): List<AttributeSnippet> = methodBase() + listOf(
        at(),
        annotationValue("slice", "Slice"),
    ) + mixinExtrasRequirementOptions() + listOf(
        plain("order"),
    )

    private fun wrapOperationBase(): List<AttributeSnippet> = mixinExtrasInjectorBase() + listOf(
        annotationValue("constant", "Constant"),
    )

    private fun wrapMethodBase(): List<AttributeSnippet> = methodBase() + mixinExtrasRequirementOptions() + listOf(
        plain("order"),
    )

    private fun isValidAttributeForContext(context: AnnotationContext, attributeName: String): Boolean {
        if (
            context.annotation == MixinAnnotation.WRAP_WITH_CONDITION &&
            attributeName == "order" &&
            context.resolvedAnnotationFqn == WRAP_WITH_CONDITION_V1_FQN
        ) {
            return false
        }
        if (context.annotation != MixinAnnotation.WRAP_OPERATION) return true
        return when (attributeName) {
            "at" -> "constant" !in context.existingAttributes
            "constant" -> "at" !in context.existingAttributes
            else -> true
        }
    }

    private fun methodBase(): List<AttributeSnippet> = listOf(quoted("method"))

    private fun requirementOptions(): List<AttributeSnippet> = mixinExtrasRequirementOptions() + listOf(
        quoted("constraints"),
    )

    private fun mixinExtrasRequirementOptions(): List<AttributeSnippet> = listOf(
        plain("require"),
        plain("expect"),
        plain("allow"),
        boolean("remap"),
    )

    private fun quoted(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = \"…\"", "$name = \"${'$'}{1}\"${'$'}0")

    private fun quotedArray(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = { \"…\" }", "$name = { \"${'$'}{1}\" }${'$'}0")

    private fun classLiteral(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = ….class", "$name = ${'$'}{1}.class${'$'}0")

    private fun plain(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = …", "$name = ${'$'}{1}${'$'}0")

    private fun boolean(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = true/false", "$name = ${'$'}{1|true,false|}${'$'}0")

    private fun enumValue(name: String, type: String): AttributeSnippet =
        AttributeSnippet(name, "$name = $type.…", "$name = $type.${'$'}{1}${'$'}0")

    private fun annotationValue(name: String, type: String): AttributeSnippet =
        AttributeSnippet(name, "$name = @$type(…)", "$name = @$type(${ '$' }{1})${'$'}0")

    private fun at(): AttributeSnippet =
        AttributeSnippet("at", "at = @At(\"…\")", "at = @At(\"${'$'}{1}\")${'$'}0")

    private fun classArray(name: String): AttributeSnippet =
        AttributeSnippet(name, "$name = { ….class }", "$name = { ${'$'}{1}.class }${'$'}0")

    private fun localArray(): AttributeSnippet =
        AttributeSnippet("local", "local = { @Local(…) }", "local = { @Local(${ '$' }{1}) }${'$'}0")

    private fun definitionsValue(): AttributeSnippet =
        AttributeSnippet(
            "value",
            "value = { @Definition(…) }",
            "value = { @Definition(id = \"${'$'}{1}\") }${'$'}0",
        )

    private fun expressionsValue(): AttributeSnippet =
        AttributeSnippet(
            "value",
            "value = { @Expression(…) }",
            "value = { @Expression(\"${'$'}{1}\") }${'$'}0",
        )
}
