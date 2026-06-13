package io.github.mcdev.fixtures

object FixturePaths {
    const val ROOT = "fixtures"

    const val FABRIC_BASIC = "$ROOT/fabric-basic"
    const val FABRIC_MIXINEXTRAS = "$ROOT/fabric-mixinextras"
    const val FORGE_BASIC = "$ROOT/forge-basic"
    const val NEOFORGE_BASIC = "$ROOT/neoforge-basic"
    const val BROKEN_DIAGNOSTICS = "$ROOT/broken-diagnostics"
    const val FABRIC_AW_AT = "$ROOT/fabric-aw-at"
    const val MULTI_SOURCE_SET = "$ROOT/multi-source-set"
    const val FABRIC_LOOM_E2E = "$ROOT/fabric-loom-e2e"
    const val SHARED_CLASSES = "$ROOT/shared/classes"

    const val FABRIC_BASIC_BUILD_GRADLE = "$FABRIC_BASIC/build.gradle"
    const val FABRIC_BASIC_FABRIC_MOD_JSON = "$FABRIC_BASIC/fabric.mod.json"
    const val FABRIC_BASIC_MIXINS_JSON = "$FABRIC_BASIC/mixins.json"
    const val FABRIC_BASIC_MAPPINGS = "$FABRIC_BASIC/mappings.tiny"
    const val FABRIC_BASIC_EXAMPLE_MIXIN = "$FABRIC_BASIC/src/main/java/com/example/mixin/ExampleMixin.java"

    const val FABRIC_MIXINEXTRAS_BUILD_GRADLE = "$FABRIC_MIXINEXTRAS/build.gradle"
    const val FABRIC_MIXINEXTRAS_FABRIC_MOD_JSON = "$FABRIC_MIXINEXTRAS/fabric.mod.json"
    const val FABRIC_MIXINEXTRAS_MIXINS_JSON = "$FABRIC_MIXINEXTRAS/mixins.json"
    const val FABRIC_MIXINEXTRAS_MIXIN = "$FABRIC_MIXINEXTRAS/src/main/java/com/example/mixin/MixinExtrasExample.java"

    const val FORGE_BASIC_BUILD_GRADLE = "$FORGE_BASIC/build.gradle"
    const val FORGE_BASIC_MIXINS_JSON = "$FORGE_BASIC/mixins.json"
    const val FORGE_BASIC_EXAMPLE_MIXIN = "$FORGE_BASIC/src/main/java/com/example/mixin/ForgeExampleMixin.java"

    const val NEOFORGE_BASIC_BUILD_GRADLE = "$NEOFORGE_BASIC/build.gradle"
    const val NEOFORGE_BASIC_MIXINS_JSON = "$NEOFORGE_BASIC/mixins.json"
    const val NEOFORGE_BASIC_EXAMPLE_MIXIN = "$NEOFORGE_BASIC/src/main/java/com/example/mixin/NeoForgeExampleMixin.java"

    const val BROKEN_DIAGNOSTICS_BUILD_GRADLE = "$BROKEN_DIAGNOSTICS/build.gradle"
    const val BROKEN_DIAGNOSTICS_FABRIC_MOD_JSON = "$BROKEN_DIAGNOSTICS/fabric.mod.json"
    const val BROKEN_DIAGNOSTICS_MIXINS_JSON = "$BROKEN_DIAGNOSTICS/mixins.json"
    const val BROKEN_DIAGNOSTICS_MIXIN = "$BROKEN_DIAGNOSTICS/src/main/java/com/example/mixin/BrokenMixin.java"

    const val FABRIC_AW_AT_BUILD_GRADLE = "$FABRIC_AW_AT/build.gradle"
    const val FABRIC_AW_AT_FABRIC_MOD_JSON = "$FABRIC_AW_AT/fabric.mod.json"
    const val FABRIC_AW_AT_MAPPINGS = "$FABRIC_AW_AT/mappings.tiny"
    const val FABRIC_AW_AT_ACCESS_WIDENER = "$FABRIC_AW_AT/src/main/resources/mod.accesswidener"
    const val FABRIC_AW_AT_ACCESS_TRANSFORMER = "$FABRIC_AW_AT/src/main/resources/mod_at.cfg"

    const val MULTI_SOURCE_SET_BUILD_GRADLE = "$MULTI_SOURCE_SET/build.gradle"
    const val MULTI_SOURCE_SET_FABRIC_MOD_JSON = "$MULTI_SOURCE_SET/fabric.mod.json"
    const val MULTI_SOURCE_SET_MIXINS_JSON = "$MULTI_SOURCE_SET/mixins.json"
    const val MULTI_SOURCE_SET_MAIN_MIXIN = "$MULTI_SOURCE_SET/src/main/java/com/example/mixin/MainMixin.java"
    const val MULTI_SOURCE_SET_CLIENT_MIXIN = "$MULTI_SOURCE_SET/src/client/java/com/example/mixin/ClientMixin.java"
    const val MULTI_SOURCE_SET_ACCESS_WIDENER = "$MULTI_SOURCE_SET/src/main/resources/mod.accesswidener"

    const val FABRIC_LOOM_E2E_BUILD_GRADLE = "$FABRIC_LOOM_E2E/build.gradle"
    const val FABRIC_LOOM_E2E_FABRIC_MOD_JSON = "$FABRIC_LOOM_E2E/fabric.mod.json"
    const val FABRIC_LOOM_E2E_MIXINS_JSON = "$FABRIC_LOOM_E2E/mixins.json"
    const val FABRIC_LOOM_E2E_MAPPINGS = "$FABRIC_LOOM_E2E/mappings.tiny"
    const val FABRIC_LOOM_E2E_EXAMPLE_MIXIN = "$FABRIC_LOOM_E2E/src/main/java/com/example/mixin/ExampleMixin.java"
    const val FABRIC_LOOM_E2E_MAPPED_SOURCE = "$FABRIC_LOOM_E2E/mapped-sources/com/example/target/SimpleTarget.java"
    const val FABRIC_LOOM_E2E_PROJECT = "$FABRIC_LOOM_E2E/.project"
    const val FABRIC_LOOM_E2E_CLASSPATH = "$FABRIC_LOOM_E2E/.classpath"

    const val SIMPLE_TARGET_CLASS = "$SHARED_CLASSES/com/example/target/SimpleTarget.class"

    val ALL_FIXTURE_ROOTS = listOf(
        FABRIC_BASIC,
        FABRIC_MIXINEXTRAS,
        FORGE_BASIC,
        NEOFORGE_BASIC,
        BROKEN_DIAGNOSTICS,
        FABRIC_AW_AT,
        MULTI_SOURCE_SET,
        FABRIC_LOOM_E2E,
    )

    val REQUIRED_RESOURCES = listOf(
        FABRIC_BASIC_BUILD_GRADLE,
        FABRIC_BASIC_FABRIC_MOD_JSON,
        FABRIC_BASIC_MIXINS_JSON,
        FABRIC_BASIC_MAPPINGS,
        FABRIC_BASIC_EXAMPLE_MIXIN,
        FABRIC_MIXINEXTRAS_BUILD_GRADLE,
        FABRIC_MIXINEXTRAS_FABRIC_MOD_JSON,
        FABRIC_MIXINEXTRAS_MIXINS_JSON,
        FABRIC_MIXINEXTRAS_MIXIN,
        FORGE_BASIC_BUILD_GRADLE,
        FORGE_BASIC_MIXINS_JSON,
        FORGE_BASIC_EXAMPLE_MIXIN,
        NEOFORGE_BASIC_BUILD_GRADLE,
        NEOFORGE_BASIC_MIXINS_JSON,
        NEOFORGE_BASIC_EXAMPLE_MIXIN,
        BROKEN_DIAGNOSTICS_BUILD_GRADLE,
        BROKEN_DIAGNOSTICS_FABRIC_MOD_JSON,
        BROKEN_DIAGNOSTICS_MIXINS_JSON,
        BROKEN_DIAGNOSTICS_MIXIN,
        FABRIC_AW_AT_BUILD_GRADLE,
        FABRIC_AW_AT_FABRIC_MOD_JSON,
        FABRIC_AW_AT_MAPPINGS,
        FABRIC_AW_AT_ACCESS_WIDENER,
        FABRIC_AW_AT_ACCESS_TRANSFORMER,
        MULTI_SOURCE_SET_BUILD_GRADLE,
        MULTI_SOURCE_SET_FABRIC_MOD_JSON,
        MULTI_SOURCE_SET_MIXINS_JSON,
        MULTI_SOURCE_SET_MAIN_MIXIN,
        MULTI_SOURCE_SET_CLIENT_MIXIN,
        MULTI_SOURCE_SET_ACCESS_WIDENER,
        FABRIC_LOOM_E2E_BUILD_GRADLE,
        FABRIC_LOOM_E2E_FABRIC_MOD_JSON,
        FABRIC_LOOM_E2E_MIXINS_JSON,
        FABRIC_LOOM_E2E_MAPPINGS,
        FABRIC_LOOM_E2E_EXAMPLE_MIXIN,
        FABRIC_LOOM_E2E_MAPPED_SOURCE,
        FABRIC_LOOM_E2E_PROJECT,
        FABRIC_LOOM_E2E_CLASSPATH,
        SIMPLE_TARGET_CLASS,
    )
}
