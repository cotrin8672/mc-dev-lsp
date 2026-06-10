package io.github.mcdev.jdtls.command

import io.github.mcdev.protocol.McdevCommands

/**
 * Static command IDs for JDT LS `workspace/executeCommand` registration.
 *
 * Register each [CommandRegistration.id] with the Eclipse command framework so
 * Neovim can invoke mcdev through the active JDT LS client.
 */
object McdevCommandRegistry {
    data class CommandRegistration(
        val id: String,
        val title: String,
        val description: String,
        val implemented: Boolean,
    )

    val commands: List<CommandRegistration> = listOf(
        CommandRegistration(
            id = McdevCommands.COMPLETION,
            title = "Minecraft Development Completion",
            description = "Returns mixin and modding completion items for the current Java buffer position.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.CONTEXT,
            title = "Minecraft Development Diagnostics",
            description = "Analyzes the current buffer and returns structured mixin diagnostics.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.INFO,
            title = "Minecraft Development Info",
            description = "Returns project platform, mappings, mixin config, and index state.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.REINDEX,
            title = "Minecraft Development Reindex",
            description = "Rebuilds the bytecode and class member index from the discovered classpath.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.DEFINITION,
            title = "Minecraft Development Definition",
            description = "Resolves mixin and modding symbol definitions.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.REFERENCES,
            title = "Minecraft Development References",
            description = "Finds mixin and modding symbol references.",
            implemented = true,
        ),
        CommandRegistration(
            id = McdevCommands.CODE_ACTION,
            title = "Minecraft Development Code Action",
            description = "Returns code actions for mixin and modding diagnostics.",
            implemented = true,
        ),
    )

    val implementedCommandIds: List<String> =
        commands.filter { it.implemented }.map { it.id }
}
