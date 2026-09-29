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
  if not action then return end
  bufnr = bufnr and bufnr ~= 0 and bufnr or vim.api.nvim_get_current_buf()
  if action.disabled then
    vim.notify(action.disabled.reason, vim.log.levels.WARN)
    return
  end
  local client_id = action._mcdev_client_id
  local client = client_id and vim.lsp.get_client_by_id(client_id) or nil
  if client_id and not client then
    vim.notify("mcdev: code action client is no longer available", vim.log.levels.WARN)
    return
  end
  local encoding = client and client.offset_encoding or "utf-16"
  local function apply(resolved)
    if resolved.edit then
      vim.lsp.util.apply_workspace_edit(resolved.edit, encoding)
    end
    if resolved.command then
      local command = type(resolved.command) == "table" and resolved.command or resolved
      local command_client = client or protocol.active_jdtls_client(bufnr)
      if command_client then
        command_client:exec_cmd(command, { bufnr = bufnr })
      else
        vim.notify("mcdev: no client available to execute code action", vim.log.levels.WARN)
      end
    end
  end
  local unresolved = vim.deepcopy(action)
  unresolved._mcdev_client_id = nil
  if client and type(action.command) ~= "string" and not (action.edit and action.command)
    and client:supports_method("codeAction/resolve", bufnr) then
    local started = client:request("codeAction/resolve", unresolved, function(err, resolved)
      if err or not resolved or resolved == vim.NIL then
        if action.edit or action.command then
          apply(unresolved)
        else
          vim.notify("mcdev: code action resolve failed: " .. tostring(err and err.message or "empty response"), vim.log.levels.WARN)
        end
        return
      end
      apply(resolved)
    end, bufnr)
    if not started then
      vim.notify("mcdev: failed to start code action resolve", vim.log.levels.WARN)
    end
  else
    apply(unresolved)
  end
end

return M
