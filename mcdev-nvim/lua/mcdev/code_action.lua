local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")
local buffer = require("mcdev.buffer")

local M = {}

function M.code_actions(bufnr, range, diagnostic_codes, cb)
  local snapshot = buffer.snapshot(bufnr)
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
      local item = convert.to_lsp_code_action(action)
      item._mcdev_snapshots = buffer.edit_snapshots(item.edit, snapshot and { [snapshot.uri] = snapshot } or {})
      table.insert(actions, item)
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
  local request_params = action._mcdev_request_params
  local snapshots = buffer.edit_snapshots(action.edit, vim.deepcopy(action._mcdev_snapshots or {}))
  if next(snapshots) == nil then
    local snapshot = buffer.snapshot(bufnr)
    if snapshot then snapshots[snapshot.uri] = snapshot end
  end
  local function apply(resolved)
    buffer.edit_snapshots(resolved.edit, snapshots)
    for _, snapshot in pairs(snapshots) do
      if not buffer.snapshot_current(snapshot) then
        vim.notify("mcdev: code action source changed; request actions again", vim.log.levels.WARN)
        return
      end
    end
    if resolved.edit then
      vim.lsp.util.apply_workspace_edit(resolved.edit, encoding)
    end
    if resolved.command then
      local command
      if type(resolved.command) == "table" then
        command = resolved.command
      else
        command = vim.deepcopy(resolved)
        command._mcdev_client_id = nil
        command._mcdev_request_params = nil
        command._mcdev_snapshots = nil
      end
      local command_client = client or protocol.active_ktlsp_client(bufnr)
      if command_client then
        local context = { bufnr = bufnr }
        if request_params then context.params = request_params end
        command_client:exec_cmd(command, context)
      else
        vim.notify("mcdev: no client available to execute code action", vim.log.levels.WARN)
      end
    end
  end
  local unresolved = vim.deepcopy(action)
  unresolved._mcdev_client_id = nil
  unresolved._mcdev_request_params = nil
  unresolved._mcdev_snapshots = nil
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
