local M = {}

M.options = {
  ktlsp = { cmd = nil },
  companion = { cmd = nil },
  insert = {
    at_target = "smart",
    mixin_class_import = true,
    inject_method_descriptor = "auto",
  },
  completion = {
    omnifunc = false,
    omnifunc_timeout_ms = 500,
  },
  standard_lsp = {
    prefer = true,
  },
  diagnostics = {
    enabled = false,
    events = { "TextChanged", "TextChangedI", "TextChangedP", "InsertLeave", "BufWritePost" },
    debounce_ms = 500,
    insert_mode = true,
    in_flight_policy = "latest",
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
  if opts and opts.jdtls then
    error("mcdev: jdtls options were replaced by ktlsp.cmd and companion.cmd (explicit executable argv)")
  end
  M.options = merge(M.options, opts or {})
end

return M
