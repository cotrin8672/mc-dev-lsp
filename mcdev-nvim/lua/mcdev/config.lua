local M = {}

M.options = {
  jdtls = {
    extension_jar = nil,
  },
  completion = {
    enable = false,
    source = "blink",
  },
  diagnostics = {
    enable = false,
    debounce_ms = 300,
  },
  navigation = {
    enable = false,
  },
  code_action = {
    enable = false,
  },
  mappings = {
    preferred_at_target = "descriptor",
    mixin_class_insert = "import",
    inject_method_descriptor = "auto",
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
