package io.github.mcdev.core.mixinextras

data class DefinitionAnnotationParseIssue(
    val attribute: String,
    val rawValue: String,
    val message: String,
    /** Half-open offset range within the @Definition body. */
    val bodyRange: IntRange,
)

data class MixinExtrasDefinition(
    val id: String? = null,
    val rawMethodReferences: List<String> = emptyList(),
    val rawFieldReferences: List<String> = emptyList(),
    val classLiteralTypeNames: List<String> = emptyList(),
    val localSpecs: List<HandlerParameterSugarSpec.Local> = emptyList(),
    val remap: Boolean? = null,
) {
    internal var bodyLength: Int = 0

    /** Syntax errors retained while valid attributes in the same annotation are parsed. */
    internal var parseIssues: List<DefinitionAnnotationParseIssue> = emptyList()
        private set

    /** Half-open annotation body span, retained even when its id is missing. */
    var sourceRange: IntRange? = null
        private set

    /** Half-open [start, end) span of the id string literal content within a @Definition body. */
    var idContentRange: IntRange? = null
        internal set

    /** Half-open [start, end) span of the id string literal content in the enclosing source file. */
    var idSourceRange: IntRange? = null
        private set

    internal fun attachIdContentRange(range: IntRange?): MixinExtrasDefinition {
        idContentRange = range
        return this
    }

    /**
     * Attaches parse issues discovered by a semantic front end.
     *
     * The JDT adapter cannot use the hand-written annotation parser, but it must
     * still pass malformed definition members to the same identifier-pool and
     * diagnostic path as the source parser.
     */
    fun withParseIssues(issues: List<DefinitionAnnotationParseIssue>): MixinExtrasDefinition {
        parseIssues = issues
        return this
    }

    /** Attaches the absolute source span of a definition body discovered by a semantic front end. */
    fun withSemanticSourceRange(range: IntRange?): MixinExtrasDefinition {
        sourceRange = range
        return this
    }

    /**
     * Maps [idContentRange] into an absolute half-open [idSourceRange] using a single file base.
     *
     * [definitionBodyStartInSource] must be the absolute offset of the first character inside
     * `@Definition(...)`, i.e. handlerRegionBaseOffset + definitionBodyStartInHandlerRegion.
     */
    fun withIdSourceRange(definitionBodyStartInSource: Int): MixinExtrasDefinition {
        sourceRange = definitionBodyStartInSource until (definitionBodyStartInSource + bodyLength)
        idSourceRange = idContentRange?.let { range ->
            val start = definitionBodyStartInSource + range.first
            start until (definitionBodyStartInSource + range.last + 1)
        }
        return this
    }

    internal fun withIdSourceRangeFromHandlerRegion(
        handlerRegionBaseOffset: Int,
        definitionBodyStartInHandlerRegion: Int,
    ): MixinExtrasDefinition = withIdSourceRange(handlerRegionBaseOffset + definitionBodyStartInHandlerRegion)
}

data class MixinExtrasDefinitionIndex(
    val definitions: List<MixinExtrasDefinition> = emptyList(),
) {
    fun definitionsWithId(id: String): List<MixinExtrasDefinition> =
        definitions.filter { it.id == id }
}
