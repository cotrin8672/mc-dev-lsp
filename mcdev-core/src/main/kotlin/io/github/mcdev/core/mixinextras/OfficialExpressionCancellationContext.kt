package io.github.mcdev.core.mixinextras

/** Supplies request-scoped cancellation to production expression operations. */
object OfficialExpressionCancellationContext {
    private val currentChecker = ThreadLocal<OfficialExpressionCancellationChecker?>()

    fun current(): OfficialExpressionCancellationChecker =
        currentChecker.get() ?: OfficialExpressionCancellationChecker.NONE

    fun <T> withChecker(
        checker: OfficialExpressionCancellationChecker,
        block: () -> T,
    ): T {
        val previous = currentChecker.get()
        currentChecker.set(checker)
        return try {
            block()
        } finally {
            if (previous == null) {
                currentChecker.remove()
            } else {
                currentChecker.set(previous)
            }
        }
    }
}
