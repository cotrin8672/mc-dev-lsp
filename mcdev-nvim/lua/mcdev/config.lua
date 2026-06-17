local M = {}

M.options = {
  jdtls = {
    extension_jar = nil,
    mason = {
      enabled = true,
      package = "mcdev-jdtls-extension",
      jar = "io.github.mcdev.jdtls.jar",
    },
  },
  insert = {
    at_target = "smart",
    mixin_class_import = true,
    inject_method_descriptor = "auto",
  },
  completion = {
    omnifunc = true,
  },
  standard_lsp = {
    prefer = true,
  },
  diagnostics = {
    enabled = false,
    events = { "BufWritePost" },
    debounce_ms = 1000,
    insert_mode = false,
    in_flight_policy = "drop",
    stale_result_policy = "drop",
  },
}

local function merge(base, override)
  local result = vim.deepcopy(base)
  for key, value in pairs(override or {}) do
    if type(value) == "table" and type(result[key]) == "table" then
      result[key] = merge(result[key], value)
    else
      result[key] = value
    end
  end
  return result
end

function M.setup(opts)
  M.options = merge(M.options, opts or {})
end

return M
