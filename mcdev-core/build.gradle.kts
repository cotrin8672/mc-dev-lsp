plugins {
    kotlin("jvm")
}

kotlin {
    jvmToolchain(21)
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
        freeCompilerArgs.add("-Xjsr305=strict")
    }
}

dependencies {
    implementation("org.ow2.asm:asm:9.7")
    implementation("org.ow2.asm:asm-tree:9.7")
    implementation("org.ow2.asm:asm-util:9.7")
    implementation("com.google.code.gson:gson:2.11.0")
    implementation("io.github.llamalad7:mixinextras-expressions:0.0.6") {
        isTransitive = false
    }
    implementation("org.spongepowered:mixin:0.8") {
        isTransitive = false
    }
    implementation("org.apache.commons:commons-lang3:3.18.0")
    implementation("com.google.guava:guava:32.1.2-jre") {
        isTransitive = false
    }
    implementation("org.ow2.asm:asm-analysis:9.7")
    implementation("org.antlr:antlr4-runtime:4.13.1")
    testImplementation(project(":mcdev-test-fixtures"))
    testImplementation(kotlin("test"))
}

tasks.test {
    useJUnitPlatform()
}
