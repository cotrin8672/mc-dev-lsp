plugins {
    kotlin("jvm")
    java
}

kotlin {
    jvmToolchain(21)
}

tasks.withType<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>().configureEach {
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }
}

dependencies {
    implementation(project(":mcdev-core"))
    implementation("com.google.code.gson:gson:2.11.0")
    testImplementation(kotlin("test"))
}

val fixtureJavaDir = layout.projectDirectory.dir("src/fixture-java")

val compileFixtureJava by tasks.registering(JavaCompile::class) {
    group = "fixtures"
    description = "Compiles minimal Java target classes for test fixtures"
    source(fixtureJavaDir)
    classpath = files()
    destinationDirectory.set(layout.buildDirectory.dir("generated/fixture-classes"))
    sourceCompatibility = JavaVersion.VERSION_21.toString()
    targetCompatibility = JavaVersion.VERSION_21.toString()
    options.compilerArgs.addAll(listOf("-Xlint:-options"))
}

tasks.named<ProcessResources>("processResources") {
    dependsOn(compileFixtureJava)
    from(compileFixtureJava.flatMap { it.destinationDirectory }) {
        into("fixtures/shared/classes")
    }
}

tasks.named("compileKotlin") {
    dependsOn(compileFixtureJava)
}

tasks.test {
    useJUnitPlatform()
}
