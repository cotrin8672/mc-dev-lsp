local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")

local M = {}

function M.code_actions(bufnr, range, diagnostic_codes, cb)
  protocol.code_action(bufnr, range, diagnostic_codes, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    local actions = {}
    for _, action in ipairs((result and result.actions) or {}) do
      table.insert(actions, convert.to_lsp_code_action(action))
    end
    if cb then
      cb(actions, nil)
    end
  end)
end

function M.apply(action)
  if action and action.edit then
    vim.lsp.util.apply_workspace_edit(action.edit, "utf-8")
  end
end

return M
