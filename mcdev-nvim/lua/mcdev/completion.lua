local protocol = require("mcdev.protocol")

local M = {}

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

function M.complete(callback)
  protocol.completion(function(envelope, err)
    if err then
      callback({ isIncomplete = false, items = {} })
      return
    end
    local result = envelope and envelope.result or { items = {} }
    local items = {}
    for _, item in ipairs(result.items or {}) do
      table.insert(items, M.to_lsp_item(item))
    end
    callback({ isIncomplete = false, items = items })
  end)
end

return M
