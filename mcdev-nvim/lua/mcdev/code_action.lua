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

function M.apply(action, bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if action and action.edit then
    local client = protocol.active_jdtls_client(bufnr)
    vim.lsp.util.apply_workspace_edit(action.edit, (client and client.offset_encoding) or "utf-16")
  end
  if action and action.command then
    local command = type(action.command) == "table" and action.command or {
      command = action.command,
      arguments = action.arguments,
    }
    if vim.lsp.buf.execute_command then
      vim.lsp.buf.execute_command(command)
    else
      local client = protocol.active_jdtls_client(bufnr)
      if client then
        client.request("workspace/executeCommand", command, nil, bufnr)
      end
    end
  end
end

return M
