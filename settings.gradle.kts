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
        exclusiveContent {
            forRepository {
                maven("https://repo.spongepowered.org/repository/maven-public/")
            }
            filter {
                includeGroup("org.spongepowered")
            }
        }
    }
}

rootProject.name = "mcdev-kotlin"

include(
    "mcdev-core",
    "mcdev-protocol",
    "mcdev-jdtls-extension",
    "mcdev-test-fixtures",
)
