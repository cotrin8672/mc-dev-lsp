package io.github.mcdev.jdtls.java

import io.github.mcdev.core.definition.McDefinitionTarget
import io.github.mcdev.core.diagnostics.McTextPosition
import io.github.mcdev.core.diagnostics.McTextRange
import io.github.mcdev.core.model.MemberKind
import io.github.mcdev.jdtls.project.UriPathSupport
import io.github.mcdev.protocol.McdevDefinitionResolution
import java.net.URI
import java.nio.file.Path

data class JdtResolvedLocation(
    val documentUri: String,
    val range: McTextRange,
    val resolution: McdevDefinitionResolution,
    val resolutionMessage: String? = null,
)

class JdtReflectionBridge private constructor() {
    private val available: Boolean
    private val javaCoreClass: Class<*>
    private val jdtUtilsClass: Class<*>?
    private val signatureClass: Class<*>?

    init {
        val core = runCatching { Class.forName("org.eclipse.jdt.core.JavaCore") }.getOrNull()
        available = core != null
        javaCoreClass = core ?: Any::class.java
        jdtUtilsClass = runCatching { Class.forName("org.eclipse.jdt.ls.core.internal.JDTUtils") }.getOrNull()
        signatureClass = runCatching { Class.forName("org.eclipse.jdt.core.Signature") }.getOrNull()
    }

    fun isAvailable(): Boolean = available

    fun resolveDefinition(target: McDefinitionTarget, workspaceRootUri: String): JdtResolvedLocation? {
        if (!available) return null
        val javaProject = findJavaProject(workspaceRootUri) ?: return null
        val element = findJavaElement(javaProject, target) ?: return null
        return toResolvedLocation(element, target)
    }

    fun discoverClasspathEntries(workspaceRootUri: String): List<Path> {
        if (!available) return emptyList()
        val javaProject = findJavaProject(workspaceRootUri) ?: return emptyList()
        return runCatching { readClasspath(javaProject) }.getOrDefault(emptyList())
    }

    private fun findJavaProject(workspaceRootUri: String): Any? {
        val rootPath = UriPathSupport.uriToPath(workspaceRootUri)
        val project = findProjectByManager(rootPath) ?: findProjectByWorkspace(rootPath)
        if (project == null) return null
        return javaCoreClass.getMethod("create", Class.forName("org.eclipse.core.resources.IProject"))
            .invoke(null, project)
    }

    private fun findProjectByManager(rootPath: Path): Any? {
        val pluginClass = runCatching {
            Class.forName("org.eclipse.jdt.ls.core.internal.JavaLanguageServerPlugin")
        }.getOrNull() ?: return null
        val plugin = pluginClass.getMethod("getInstance").invoke(null) ?: return null
        val manager = pluginClass.getMethod("getProjectsManager").invoke(plugin) ?: return null
        val uri = rootPath.toUri()
        return manager.javaClass.methods.firstOrNull { method ->
            method.name == "getProject" && method.parameterCount == 1
        }?.invoke(manager, uri)
    }

    private fun findProjectByWorkspace(rootPath: Path): Any? {
        val resourcesPlugin = runCatching {
            Class.forName("org.eclipse.core.resources.ResourcesPlugin")
        }.getOrNull() ?: return null
        val workspace = resourcesPlugin.getMethod("getWorkspace").invoke(null) ?: return null
        val root = workspace.javaClass.getMethod("getRoot").invoke(workspace) ?: return null
        val projects = root.javaClass.getMethod("getProjects").invoke(root) as? Array<*> ?: return null
        val normalizedRoot = rootPath.toAbsolutePath().normalize()
        for (candidate in projects) {
            if (candidate == null) continue
            val exists = candidate.javaClass.getMethod("exists").invoke(candidate) as? Boolean ?: false
            if (!exists) continue
            val location = candidate.javaClass.getMethod("getLocation").invoke(candidate) ?: continue
            val locationPath = location.javaClass.getMethod("toPath").invoke(location) as? java.nio.file.Path
            if (locationPath != null && locationPath.toAbsolutePath().normalize() == normalizedRoot) {
                return candidate
            }
        }
        val projectName = readEclipseProjectName(rootPath) ?: rootPath.fileName?.toString() ?: return null
        val project = root.javaClass.getMethod("getProject", String::class.java).invoke(root, projectName)
        val exists = project?.javaClass?.getMethod("exists")?.invoke(project) as? Boolean ?: false
        return project.takeIf { exists }
    }

    private fun readEclipseProjectName(rootPath: Path): String? {
        val projectFile = rootPath.resolve(".project")
        if (!projectFile.toFile().isFile) return null
        val text = runCatching { projectFile.toFile().readText() }.getOrNull() ?: return null
        return ECLIPSE_PROJECT_NAME_PATTERN.find(text)?.groupValues?.get(1)
    }

    private fun findJavaElement(javaProject: Any, target: McDefinitionTarget): Any? {
        val ownerFqn = target.ownerFqn ?: target.ownerInternalName.replace('/', '.')
        val iType = javaProject.javaClass.getMethod("findType", String::class.java)
            .invoke(javaProject, ownerFqn) ?: return null
        return when (target.kind) {
            MemberKind.CLASS -> iType
            MemberKind.FIELD -> {
                val name = target.name ?: return null
                iType.javaClass.getMethod("getField", String::class.java).invoke(iType, name)
            }
            MemberKind.METHOD -> {
                val name = target.name ?: return null
                val methods = iType.javaClass.getMethod("getMethods").invoke(iType) as? Array<*> ?: return null
                selectMethod(methods, name, target.descriptor)
            }
            else -> null
        }
    }

    private fun selectMethod(methods: Array<*>, name: String, descriptor: String?): Any? {
        val matches = methods.filter { method ->
            val elementName = method?.javaClass?.getMethod("getElementName")?.invoke(method) as? String
            elementName == name
        }
        if (matches.isEmpty()) return null
        val signature = signatureClass
        if (descriptor.isNullOrBlank() || signature == null) {
            return matches.singleOrNull() ?: matches.first()
        }
        val expectedParams = runCatching {
            signature.getMethod("getParameterTypes", String::class.java)
                .invoke(null, descriptor) as? Array<*>
        }.getOrNull()
        if (expectedParams == null) return matches.first()
        return matches.firstOrNull { method ->
            val actualParams = method?.javaClass?.getMethod("getParameterTypes")?.invoke(method) as? Array<*>
            actualParams != null && actualParams.contentDeepEquals(expectedParams)
        } ?: matches.first()
    }

    private fun toResolvedLocation(element: Any, target: McDefinitionTarget): JdtResolvedLocation? {
        jdtUtilsClass?.let { utils ->
            val location = utils.methods.firstOrNull { it.name == "toLocation" && it.parameterCount == 1 }
                ?.invoke(null, element)
            if (location != null) {
                return locationToResolved(location, McdevDefinitionResolution.JDT)
            }
        }
        return manualLocation(element, target)
    }

    private fun locationToResolved(location: Any, resolution: McdevDefinitionResolution): JdtResolvedLocation? {
        val uri = location.javaClass.getMethod("getUri").invoke(location)?.toString() ?: return null
        val range = location.javaClass.getMethod("getRange").invoke(location) ?: return null
        val start = range.javaClass.getMethod("getStart").invoke(range) ?: return null
        val end = range.javaClass.getMethod("getEnd").invoke(range) ?: return null
        return JdtResolvedLocation(
            documentUri = uri,
            range = McTextRange(
                start = positionFromLsp(start),
                end = positionFromLsp(end),
            ),
            resolution = resolution,
        )
    }

    private fun positionFromLsp(position: Any): McTextPosition {
        val line = position.javaClass.getMethod("getLine").invoke(position) as Int
        val character = position.javaClass.getMethod("getCharacter").invoke(position) as Int
        return McTextPosition(line = line, character = character)
    }

    private fun manualLocation(element: Any, target: McDefinitionTarget): JdtResolvedLocation? {
        val resource = element.javaClass.getMethod("getResource").invoke(element) ?: return null
        val exists = resource.javaClass.getMethod("exists").invoke(resource) as? Boolean ?: false
        if (!exists) {
            return JdtResolvedLocation(
                documentUri = "",
                range = target.sourceRange ?: McTextRange(McTextPosition(0, 0), McTextPosition(0, 0)),
                resolution = McdevDefinitionResolution.BYTECODE_ONLY,
                resolutionMessage = "JDT element has no attached source",
            )
        }
        val locationUri = resource.javaClass.getMethod("getLocationURI").invoke(resource) as? URI
        val documentUri = locationUri?.toString() ?: return null
        val nameRange = runCatching {
            element.javaClass.getMethod("getNameRange").invoke(element)
        }.getOrNull()
        val nameOffset = sourceRangeOffset(nameRange) ?: return JdtResolvedLocation(
            documentUri = documentUri,
            range = McTextRange(McTextPosition(0, 0), McTextPosition(0, 1)),
            resolution = McdevDefinitionResolution.JDT,
        )
        val sourceRange = runCatching {
            readSourceRange(element, nameOffset)
        }.getOrNull() ?: McTextRange(McTextPosition(0, 0), McTextPosition(0, 1))
        return JdtResolvedLocation(
            documentUri = documentUri,
            range = sourceRange,
            resolution = McdevDefinitionResolution.JDT,
        )
    }

    private fun sourceRangeOffset(range: Any?): Int? {
        if (range == null) return null
        return runCatching { range.javaClass.getMethod("getOffset").invoke(range) as? Int }.getOrNull()
            ?: (range as? Int)
    }

    private fun readSourceRange(element: Any, offset: Int): McTextRange {
        val contents = readElementSource(element)
        val start = offset.coerceIn(0, contents.length)
        val end = (start + 1).coerceAtMost(contents.length)
        val before = contents.substring(0, start)
        val line = before.count { it == '\n' }
        val lineStart = before.lastIndexOf('\n') + 1
        val endBefore = contents.substring(0, end)
        val endLine = endBefore.count { it == '\n' }
        val endLineStart = endBefore.lastIndexOf('\n') + 1
        return McTextRange(
            start = McTextPosition(line, start - lineStart),
            end = McTextPosition(endLine, end - endLineStart),
        )
    }

    private fun readElementSource(element: Any): String {
        runCatching {
            element.javaClass.getMethod("getSource").invoke(element) as? String
        }.getOrNull()?.let { return it }

        val openable = runCatching {
            element.javaClass.getMethod("getOpenable").invoke(element)
        }.getOrNull()
        val buffer = openable?.let {
            runCatching { it.javaClass.getMethod("getBuffer").invoke(it) }.getOrNull()
        }
        return buffer?.let {
            runCatching { it.javaClass.getMethod("getContents").invoke(it) as? String }.getOrNull()
        } ?: ""
    }

    private fun readClasspath(javaProject: Any): List<Path> {
        val rawClasspath = runCatching {
            javaProject.javaClass.getMethod("getResolvedClasspath", Boolean::class.javaPrimitiveType)
                .invoke(javaProject, true) as? Array<*>
        }.getOrNull() ?: runCatching {
            javaProject.javaClass.getMethod("getRawClasspath").invoke(javaProject) as? Array<*>
        }.getOrNull() ?: return emptyList()
        val paths = linkedSetOf<Path>()
        val projectRoot = projectRoot(javaProject)
        for (entry in rawClasspath) {
            if (entry == null) continue
            readClasspathPath(entry, "getOutputLocation", projectRoot)?.let { paths.add(it) }
            val path = entry.javaClass.getMethod("getPath").invoke(entry) ?: continue
            val pathString = path.javaClass.getMethod("toOSString").invoke(path) as? String ?: continue
            resolveClasspathPath(pathString, projectRoot)?.let { paths.add(it) }
        }
        return paths.toList()
    }

    private fun readClasspathPath(entry: Any, methodName: String, projectRoot: Path?): Path? {
        val path = runCatching { entry.javaClass.getMethod(methodName).invoke(entry) }.getOrNull() ?: return null
        val pathString = runCatching { path.javaClass.getMethod("toOSString").invoke(path) as? String }.getOrNull()
            ?: return null
        return resolveClasspathPath(pathString, projectRoot)
    }

    private fun resolveClasspathPath(pathString: String, projectRoot: Path?): Path? {
        val direct = runCatching { Path.of(pathString) }.getOrNull()
        if (direct != null && direct.toFile().exists()) return direct
        if (projectRoot == null) return null

        val normalized = pathString.replace('\\', '/').trimStart('/')
        val segments = normalized.split('/').filter { it.isNotBlank() }
        val relativeSegments = if (segments.firstOrNull() == projectRoot.fileName?.toString()) {
            segments.drop(1)
        } else {
            segments
        }
        val projectRelative = relativeSegments.fold(projectRoot) { base, segment -> base.resolve(segment) }
        return projectRelative.takeIf { it.toFile().exists() }
    }

    private fun projectRoot(javaProject: Any): Path? {
        val project = runCatching { javaProject.javaClass.getMethod("getProject").invoke(javaProject) }.getOrNull()
            ?: return null
        val location = runCatching { project.javaClass.getMethod("getLocation").invoke(project) }.getOrNull()
            ?: return null
        return runCatching { location.javaClass.getMethod("toPath").invoke(location) as? Path }.getOrNull()
    }

    companion object {
        private val ECLIPSE_PROJECT_NAME_PATTERN = Regex("""<name>\s*([^<]+?)\s*</name>""")

        val instance: JdtReflectionBridge? by lazy {
            runCatching { JdtReflectionBridge() }.getOrNull()?.takeIf { it.isAvailable() }
        }
    }
}
