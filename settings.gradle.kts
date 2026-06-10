pluginManagement {
    repositories {
        gradlePluginPortal()
        mavenCentral()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        mavenCentral()
    }
}

rootProject.name = "mcdev-kotlin"

include(
    "mcdev-core",
    "mcdev-protocol",
    "mcdev-jdtls-extension",
    "mcdev-test-fixtures",
)
