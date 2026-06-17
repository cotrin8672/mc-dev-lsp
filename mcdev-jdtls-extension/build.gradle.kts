import java.util.jar.JarFile

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
        freeCompilerArgs.add("-Xjsr305=strict")
    }
}

sourceSets {
    val eclipseStub by creating {
        java.setSrcDirs(listOf("src/eclipseStub/java"))
    }
    main {
        compileClasspath += eclipseStub.output
    }
    test {
        compileClasspath += eclipseStub.output
        runtimeClasspath += eclipseStub.output
    }
}

tasks.named<JavaCompile>("compileEclipseStubJava") {
    sourceCompatibility = JavaVersion.VERSION_21.toString()
    targetCompatibility = JavaVersion.VERSION_21.toString()
}

tasks.named<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>("compileKotlin") {
    dependsOn(tasks.named("compileEclipseStubJava"))
}

tasks.named<org.jetbrains.kotlin.gradle.tasks.KotlinCompile>("compileTestKotlin") {
    dependsOn(tasks.named("compileEclipseStubJava"))
}

val bundleRuntimeClasspath by configurations.creating {
    isCanBeResolved = true
    isCanBeConsumed = false
}

dependencies {
    implementation(project(":mcdev-core"))
    implementation(project(":mcdev-protocol"))
    implementation("com.google.code.gson:gson:2.11.0")

    bundleRuntimeClasspath(project(":mcdev-core"))
    bundleRuntimeClasspath(project(":mcdev-protocol"))
    bundleRuntimeClasspath("com.google.code.gson:gson:2.11.0")
    bundleRuntimeClasspath(kotlin("stdlib"))

    compileOnly("org.osgi:org.osgi.core:6.0.0")
    compileOnly("org.eclipse.lsp4j:org.eclipse.lsp4j:0.23.1")

    testImplementation(project(":mcdev-test-fixtures"))
    testImplementation(kotlin("test"))
    testImplementation("org.junit.jupiter:junit-jupiter-api:5.11.4")
    testRuntimeOnly("org.junit.jupiter:junit-jupiter-engine:5.11.4")
}

tasks.jar {
    archiveBaseName.set("io.github.mcdev.jdtls")
    duplicatesStrategy = DuplicatesStrategy.EXCLUDE

    from(sourceSets.main.get().output) {
        exclude("org/eclipse/**")
    }

    dependsOn(":mcdev-core:jar", ":mcdev-protocol:jar")
    bundleRuntimeClasspath.forEach { artifact ->
        from(if (artifact.isDirectory) artifact else zipTree(artifact)) {
            exclude(
                "META-INF/MANIFEST.MF",
                "META-INF/*.SF",
                "META-INF/*.DSA",
                "META-INF/*.RSA",
            )
        }
    }

    manifest {
        attributes(
            "Bundle-ManifestVersion" to "2",
            "Bundle-SymbolicName" to "io.github.mcdev.jdtls;singleton:=true",
            "Bundle-Name" to "Minecraft Development for JDT LS",
            "Bundle-Version" to project.version.toString().removeSuffix("-SNAPSHOT"),
            "Bundle-Activator" to "io.github.mcdev.jdtls.McdevPlugin",
            "Bundle-ActivationPolicy" to "lazy",
            "Bundle-RequiredExecutionEnvironment" to "JavaSE-21",
            "Bundle-ClassPath" to ".",
            "Require-Bundle" to listOf(
                "org.eclipse.core.runtime",
                "org.eclipse.core.resources",
                "org.eclipse.jdt.core",
                "org.eclipse.jdt.ls.core",
            ).joinToString(","),
            "Private-Package" to "io.github.mcdev.*",
            "Import-Package" to "org.eclipse.lsp4j.*,org.osgi.framework,*",
        )
    }
}

tasks.register("checkBundle") {
    dependsOn(tasks.jar)
    doLast {
        val jarFile = tasks.jar.get().archiveFile.get().asFile
        JarFile(jarFile).use { jar ->
            val manifest = jar.manifest.mainAttributes
            val symbolicName = manifest.getValue("Bundle-SymbolicName") ?: ""
            check(symbolicName.startsWith("io.github.mcdev.jdtls")) {
                "unexpected Bundle-SymbolicName: $symbolicName"
            }
            check(manifest.getValue("Bundle-Activator") == "io.github.mcdev.jdtls.McdevPlugin")
            val requireBundle = manifest.getValue("Require-Bundle") ?: ""
            check("org.eclipse.jdt.core" in requireBundle) {
                "Require-Bundle must expose org.eclipse.jdt.core for ASTParser"
            }
            check("org.eclipse.core.resources" in requireBundle) {
                "Require-Bundle must expose org.eclipse.core.resources for IFile lookup"
            }

            val entries = jar.entries().asSequence().map { it.name }.toSet()
            check("plugin.xml" in entries) { "plugin.xml is missing from bundle jar" }
            check(
                entries.any { it == "io/github/mcdev/jdtls/McdevDelegateCommandHandler.class" },
            ) { "McdevDelegateCommandHandler is missing from bundle jar" }
            check(
                entries.any { it.startsWith("io/github/mcdev/core/") },
            ) { "mcdev-core classes are missing from bundle jar" }
            check(
                entries.any { it.startsWith("kotlin/") },
            ) { "kotlin stdlib classes are missing from bundle jar" }
            check(
                entries.any { it.startsWith("io/github/mcdev/protocol/") },
            ) { "mcdev-protocol classes are missing from bundle jar" }
            check(
                entries.any { it.startsWith("com/google/gson/") },
            ) { "gson classes are missing from bundle jar" }
            val pluginXml = jar.getInputStream(jar.getJarEntry("plugin.xml")!!).bufferedReader().readText()
            check("mcdev.diagnostics" in pluginXml) { "plugin.xml is missing mcdev.diagnostics command" }
            check("mcdev.definition" in pluginXml) { "plugin.xml is missing mcdev.definition command" }
            check("mcdev.references" in pluginXml) { "plugin.xml is missing mcdev.references command" }
            check("mcdev.hover" in pluginXml) { "plugin.xml is missing mcdev.hover command" }
            check("mcdev.reloadProjectContext" in pluginXml) {
                "plugin.xml is missing mcdev.reloadProjectContext command"
            }
            check("mcdev.dumpContext" in pluginXml) { "plugin.xml is missing mcdev.dumpContext command" }
        }
    }
}

tasks.test {
    useJUnitPlatform()
    dependsOn(tasks.named("eclipseStubClasses"), tasks.named("checkBundle"))
    classpath = sourceSets.test.get().runtimeClasspath
}
