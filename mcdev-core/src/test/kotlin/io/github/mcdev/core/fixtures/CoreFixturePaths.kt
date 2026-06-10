package io.github.mcdev.core.fixtures

/**
 * Path constants mirrored from [io.github.mcdev.fixtures.FixturePaths].
 *
 * mcdev-core tests can depend on `:mcdev-test-fixtures` and use the canonical
 * definitions from that module; these constants keep paths stable across modules.
 */
object CoreFixturePaths {
    const val ROOT = "fixtures"

    const val FABRIC_BASIC = "$ROOT/fabric-basic"
    const val FABRIC_MIXINEXTRAS = "$ROOT/fabric-mixinextras"
    const val FORGE_BASIC = "$ROOT/forge-basic"
    const val NEOFORGE_BASIC = "$ROOT/neoforge-basic"
    const val BROKEN_DIAGNOSTICS = "$ROOT/broken-diagnostics"
    const val SHARED_CLASSES = "$ROOT/shared/classes"

    const val FABRIC_BASIC_MAPPINGS = "$FABRIC_BASIC/mappings.tiny"
    const val FABRIC_BASIC_MIXINS_JSON = "$FABRIC_BASIC/mixins.json"
    const val SIMPLE_TARGET_CLASS = "$SHARED_CLASSES/com/example/target/SimpleTarget.class"
}
