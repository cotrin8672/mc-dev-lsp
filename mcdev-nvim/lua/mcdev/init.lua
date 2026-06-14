local config = require("mcdev.config")
local protocol = require("mcdev.protocol")
local diagnostics = require("mcdev.diagnostics")

local M = {}

function M.setup(opts)
  config.setup(opts or {})
  diagnostics.setup_autocmds()
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

return M
