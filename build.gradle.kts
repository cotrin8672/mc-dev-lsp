plugins {
    kotlin("jvm") version "2.0.21" apply false
}

group = "io.github.mcdev"
version = "0.3.0"

subprojects {
    group = rootProject.group
    version = rootProject.version
}
