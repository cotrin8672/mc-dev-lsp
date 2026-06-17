local config = require("mcdev.config")
local protocol = require("mcdev.protocol")
local diagnostics = require("mcdev.diagnostics")
local health = require("mcdev.health")

local M = {}

function M.setup(opts)
  config.setup(opts or {})
  if config.options.diagnostics.enabled then
    diagnostics.setup_autocmds(config.options.diagnostics)
  end
  vim.api.nvim_create_user_command("McdevInfo", function()
    protocol.info()
  end, {})
  vim.api.nvim_create_user_command("McdevReindex", function()
    protocol.reindex()
  end, {})
  vim.api.nvim_create_user_command("McdevReloadProjectContext", function()
    protocol.reload_project_context()
  end, {})
  vim.api.nvim_create_user_command("McdevDumpContext", function()
    protocol.dump_context()
  end, {})
  vim.api.nvim_create_user_command("McdevDiagnosticsRefresh", function()
    diagnostics.refresh(vim.api.nvim_get_current_buf(), { manual = true })
  end, {})
  vim.api.nvim_create_user_command("McdevDiagnosticsStop", function()
    diagnostics.stop()
  end, {})
  vim.api.nvim_create_user_command("McdevDiagnosticsStart", function()
    diagnostics.start()
  end, {})
  vim.api.nvim_create_user_command("McdevDiagnosticsStatus", function()
    vim.notify(table.concat(diagnostics.status_lines(), "\n"))
  end, {})
  vim.api.nvim_create_user_command("McdevHealth", function()
    health.health()
  end, {})
  vim.api.nvim_create_user_command("McdevDebugCompletion", function()
    health.debug_completion()
  end, {})
  vim.api.nvim_create_user_command("McdevDebugDiagnostics", function()
    health.debug_diagnostics()
  end, {})
end

function M.extension_jar()
  return require("mcdev.jdtls").resolve_extension_jar()
end

function M.options()
  return config.options
end

M.navigation = require("mcdev.navigation")
M.code_action = require("mcdev.code_action")
M.hover = require("mcdev.hover")
M.diagnostics = diagnostics
M.convert = require("mcdev.convert")
M.attach = require("mcdev.attach")
M.lsp = require("mcdev.lsp")

return M
