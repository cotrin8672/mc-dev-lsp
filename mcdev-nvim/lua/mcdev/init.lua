local config = require("mcdev.config")
local protocol = require("mcdev.protocol")
local buffer = require("mcdev.buffer")
local diagnostics = require("mcdev.diagnostics")

local M = {}

local function setup_completion()
  local opts = config.options.completion
  if not opts.enable then
    return
  end

  if opts.source == "omnifunc" then
    vim.api.nvim_create_autocmd("FileType", {
      callback = function(args)
        if buffer.is_mcdev_buffer(args.buf) then
          vim.bo[args.buf].omnifunc = "v:lua.require'mcdev.omnifunc'.complete"
        end
      end,
    })
    return
  end

  if opts.source == "blink" then
    local ok, blink_cmp = pcall(require, "blink.cmp")
    if ok and blink_cmp.add_source then
      blink_cmp.add_source("mcdev", require("mcdev.blink").new())
    end
    return
  end

  if opts.source == "cmp" then
    local ok, cmp = pcall(require, "cmp")
    if ok and cmp.register_source then
      cmp.register_source("mcdev", require("mcdev.cmp").new())
    end
  end
end

function M.setup(opts)
  config.setup(opts or {})
  setup_completion()
  if config.options.diagnostics.enable then
    diagnostics.setup_autocmds()
  end
  vim.api.nvim_create_user_command("McdevInfo", function()
    protocol.info()
  end, {})
  vim.api.nvim_create_user_command("McdevReindex", function()
    protocol.reindex()
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
M.diagnostics = diagnostics
M.convert = require("mcdev.convert")
M.attach = require("mcdev.attach")

return M
