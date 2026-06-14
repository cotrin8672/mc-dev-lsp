local config = require("mcdev.config")

local M = {}

local function path_join(...)
  return table.concat({ ... }, "/")
end

local function readable(path)
  return path and vim.fn.filereadable(path) == 1
end

local function default_jdtls_cmd()
  local mason_bin = vim.fn.stdpath("data") .. "/mason/bin"
  local candidates = {
    path_join(mason_bin, "jdtls"),
    path_join(mason_bin, "jdtls.cmd"),
    path_join(mason_bin, "jdtls.bat"),
  }
  for _, candidate in ipairs(candidates) do
    if vim.fn.executable(candidate) == 1 then
      return candidate
    end
  end
  if vim.fn.executable("jdtls") == 1 then
    return "jdtls"
  end
  return nil
end

local function default_root_dir()
  local markers = { "gradlew", "build.gradle", "build.gradle.kts", "pom.xml", ".git" }
  local ok, jdtls_setup = pcall(require, "jdtls.setup")
  if ok and jdtls_setup.find_root then
    local root = jdtls_setup.find_root(markers)
    if root and root ~= "" then
      return root
    end
  end

  if vim.fs and vim.fs.root then
    local bufname = vim.api.nvim_buf_get_name(0)
    local start = bufname ~= "" and bufname or vim.fn.getcwd()
    local root = vim.fs.root(start, markers)
    if root and root ~= "" then
      return root
    end
  end

  return vim.fn.getcwd()
end

local function mason_root()
  if vim.env.MASON and vim.env.MASON ~= "" then
    return vim.env.MASON
  end
  return vim.fn.stdpath("data") .. "/mason"
end

local function mason_candidates(opts)
  local mason = opts or config.options.jdtls.mason or {}
  local package = mason.package or "mcdev-jdtls-extension"
  local jar = mason.jar or "io.github.mcdev.jdtls.jar"
  local root = mason.root or mason_root()
  return {
    path_join(root, "share", package, jar),
    path_join(root, "packages", package, jar),
  }
end

function M.resolve_extension_jar(opts)
  opts = opts or {}
  local explicit = opts.extension_jar or config.options.jdtls.extension_jar
  if explicit and explicit ~= "" then
    return explicit
  end

  if readable(vim.env.MCDEV_JDTLS_EXTENSION_JAR) then
    return vim.env.MCDEV_JDTLS_EXTENSION_JAR
  end

  local mason = opts.mason or config.options.jdtls.mason or {}
  if mason.enabled == false then
    return explicit
  end

  for _, candidate in ipairs(mason_candidates(mason)) do
    if readable(candidate) then
      return candidate
    end
  end

  return explicit
end

local function missing_jar_message()
  local mason = config.options.jdtls.mason or {}
  local package = mason.package or "mcdev-jdtls-extension"
  return "mcdev: extension jar is not configured or readable; ensure Mason installs "
    .. package
    .. " or set jdtls.extension_jar"
end

function M.extend_config(jdtls_config, opts)
  jdtls_config = jdtls_config or {}
  opts = opts or {}
  local extension_jar = M.resolve_extension_jar(opts)
  if not readable(extension_jar) then
    vim.notify(missing_jar_message(), vim.log.levels.ERROR)
    return nil
  end

  jdtls_config.init_options = jdtls_config.init_options or {}
  local bundles = jdtls_config.init_options.bundles or {}
  for _, bundle in ipairs(bundles) do
    if bundle == extension_jar then
      return jdtls_config
    end
  end
  table.insert(bundles, extension_jar)
  jdtls_config.init_options.bundles = bundles
  return jdtls_config
end

function M.start_or_attach(opts)
  opts = opts or {}
  local root_dir = opts.root_dir or default_root_dir()
  local extension_jar = M.resolve_extension_jar(opts)
  if not extension_jar or vim.fn.filereadable(extension_jar) ~= 1 then
    vim.notify(missing_jar_message(), vim.log.levels.ERROR)
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

  local init_options = vim.deepcopy(opts.init_options or {})
  init_options.bundles = init_options.bundles or {}
  local has_bundle = false
  for _, bundle in ipairs(init_options.bundles) do
    if bundle == extension_jar then
      has_bundle = true
      break
    end
  end
  if not has_bundle then
    table.insert(init_options.bundles, extension_jar)
  end

  local start_opts = vim.tbl_extend("force", {
    name = "jdtls",
    cmd = cmd,
    root_dir = root_dir,
    init_options = init_options,
    capabilities = vim.lsp.protocol.make_client_capabilities(),
  }, opts)
  start_opts.cmd = cmd
  start_opts.root_dir = root_dir
  start_opts.init_options = init_options
  start_opts.extension_jar = nil
  start_opts.data_dir = nil
  start_opts.mason = nil

  return vim.lsp.start(start_opts)
end

return M
