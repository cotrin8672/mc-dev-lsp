local config = require("mcdev.config")
local protocol = require("mcdev.protocol")

local M = {}

function M.setup(opts)
  config.setup(opts or {})
  vim.api.nvim_create_user_command("McdevInfo", function()
    protocol.info()
  end, {})
  vim.api.nvim_create_user_command("McdevReindex", function()
    protocol.reindex()
  end, {})
end

function M.extension_jar()
  return config.options.jdtls.extension_jar
end

function M.options()
  return config.options
end

return M
