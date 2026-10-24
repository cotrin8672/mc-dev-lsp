local config = require("mcdev.config")
local M = { provider_id = "mcdev.mixin" }

local function argv(value)
  if type(value) ~= "table" or #value == 0 then return nil end
  for _, part in ipairs(value) do
    if type(part) ~= "string" or part == "" or part:find("\0", 1, true) then return nil end
  end
  return vim.deepcopy(value)
end

function M.companion_command(opts)
  opts = opts or {}
  local command = opts.companion and opts.companion.cmd or config.options.companion.cmd
  return argv(command)
end

function M.extend_config(client_config, opts)
  client_config = client_config or {}
  opts = opts or {}
  local init = vim.deepcopy(client_config.init_options or {})
  init.analysisProviders = init.analysisProviders or {}
  local provider = vim.deepcopy(init.analysisProviders[M.provider_id] or {})
  if opts.companion and opts.companion.cmd ~= nil and not argv(opts.companion.cmd) then
    return nil, "mcdev: companion.cmd must be a nonempty executable argv list"
  end
  if not (opts.companion and opts.companion.cmd) and provider.command ~= nil and not argv(provider.command) then
    return nil, "mcdev: existing companion provider command must be an executable argv list"
  end
  local command = opts.companion and argv(opts.companion.cmd)
    or argv(provider.command) or M.companion_command(opts)
  if not command then
    return nil, "mcdev: configure companion.cmd with explicit executable argv"
  end
  provider.command = command
  if provider.enabled == nil then provider.enabled = true end
  provider.configuration = provider.configuration or {}
  init.analysisProviders[M.provider_id] = provider
  client_config.init_options = init
  return client_config
end

local function root_dir()
  local name = vim.api.nvim_buf_get_name(0)
  local root = vim.fs.root(name ~= "" and name or vim.fn.getcwd(),
    { "gradlew", "build.gradle", "build.gradle.kts", "pom.xml", ".git" })
  return root or vim.fn.getcwd()
end

function M.start_or_attach(opts)
  opts = vim.deepcopy(opts or {})
  local command = argv(opts.cmd or config.options.ktlsp.cmd)
  if not command or vim.fn.executable(command[1]) ~= 1 then
    vim.notify("mcdev: configure ktlsp.cmd with an available executable argv", vim.log.levels.ERROR)
    return nil
  end
  local start = vim.tbl_extend("force", {
    name = "ktlsp", capabilities = vim.lsp.protocol.make_client_capabilities(),
  }, opts)
  start.cmd = command
  start.root_dir = vim.fs.normalize(vim.fn.fnamemodify(opts.root_dir or root_dir(), ":p")):gsub("/+$", "")
  local extended, err = M.extend_config(start, opts)
  if not extended then
    vim.notify(err, vim.log.levels.ERROR)
    return nil
  end
  start.companion = nil
  return vim.lsp.start(start)
end

return M
