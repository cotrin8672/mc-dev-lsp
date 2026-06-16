local protocol = require("mcdev.protocol")

local M = {}
M.last_request = nil
M.last_response_count = nil
M.last_error = nil

local kind_map = {
  class = vim.lsp.protocol.CompletionItemKind.Class,
  method = vim.lsp.protocol.CompletionItemKind.Method,
  field = vim.lsp.protocol.CompletionItemKind.Field,
  keyword = vim.lsp.protocol.CompletionItemKind.Keyword,
  value = vim.lsp.protocol.CompletionItemKind.Value,
}

function M.to_lsp_item(item)
  return {
    label = item.label,
    detail = item.detail,
    documentation = item.documentation,
    filterText = item.filterText,
    insertText = item.insertText,
    sortText = item.sortKey,
    kind = kind_map[item.kind] or vim.lsp.protocol.CompletionItemKind.Text,
    textEdit = item.edit,
    additionalTextEdits = item.additionalEdits,
    data = item.metadata,
  }
end

local function changedtick(bufnr)
  if not vim.api.nvim_buf_is_valid(bufnr) then
    return nil
  end
  return vim.api.nvim_buf_get_changedtick(bufnr)
end

function M.complete(callback, bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local request_tick = changedtick(bufnr)
  M.last_request = {
    bufnr = bufnr,
    position = position,
    changedtick = request_tick,
  }
  M.last_error = nil
  protocol.completion(function(envelope, err)
    if err then
      M.last_error = tostring(err)
      callback({ isIncomplete = false, items = {} })
      return
    end
    if request_tick ~= changedtick(bufnr) then
      callback({ isIncomplete = false, items = {} })
      return
    end
    local result = envelope and envelope.result or { items = {} }
    local items = {}
    for _, item in ipairs(result.items or {}) do
      table.insert(items, M.to_lsp_item(item))
    end
    M.last_response_count = #items
    callback({ isIncomplete = result.isIncomplete or false, items = items })
  end, bufnr, position)
end

return M
