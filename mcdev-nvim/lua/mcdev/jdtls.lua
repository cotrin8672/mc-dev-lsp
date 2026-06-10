local config = require("mcdev.config")

local M = {}

local function default_jdtls_cmd()
  local mason_cmd = vim.fn.stdpath("data") .. "/mason/bin/jdtls"
  if vim.fn.executable(mason_cmd) == 1 then
    return mason_cmd
  end
  if vim.fn.executable("jdtls") == 1 then
    return "jdtls"
  end
  return nil
end

function M.start_or_attach(opts)
  opts = opts or {}
  local root_dir = opts.root_dir or vim.fn.getcwd()
  local extension_jar = opts.extension_jar or config.options.jdtls.extension_jar
  if not extension_jar or vim.fn.filereadable(extension_jar) ~= 1 then
    vim.notify("mcdev: extension jar is not configured or readable", vim.log.levels.ERROR)
    return nil
  end

  local cmd = opts.cmd
  if not cmd then
    local jdtls_cmd = default_jdtls_cmd()
    if not jdtls_cmd then
      vim.notify("mcdev: jdtls executable not found", vim.log.levels.ERROR)
      return nil
    end
    local data_dir = opts.data_dir or (vim.fn.stdpath("cache") .. "/mcdev-jdtls")
    vim.fn.mkdir(data_dir, "p")
    cmd = { jdtls_cmd, "-data", data_dir }
  end

  local init_options = vim.tbl_deep_extend("force", opts.init_options or {}, {
    bundles = { extension_jar },
  })

  return vim.lsp.start(vim.tbl_extend("force", {
    name = "jdtls",
    cmd = cmd,
    root_dir = root_dir,
    init_options = init_options,
    capabilities = vim.lsp.protocol.make_client_capabilities(),
  }, opts))
end

return M
