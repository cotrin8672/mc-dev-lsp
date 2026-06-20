plugins {
    kotlin("jvm") version "2.0.21" apply false
}

group = "io.github.mcdev"
version = "0.7.2"

subprojects {
    group = rootProject.group
    version = rootProject.version
}
