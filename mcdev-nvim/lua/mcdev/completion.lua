local protocol = require("mcdev.protocol")
local convert = require("mcdev.convert")

local M = {}
M.last_request = nil
M.last_response_count = nil
M.last_error = nil
M.last_debug = nil
M.request_count = 0
M.server_request_count = 0
M.request_seq = 0
M.stale_dropped_count = 0
M.last_source = nil
M.last_callback_item_count = nil
M.last_local_prefix_cache_hit = false
M.last_local_prefix_cache_items = 0

local prefix_cache = nil
local active_requests = {}

local kind_map = {
  class = vim.lsp.protocol.CompletionItemKind.Class,
  method = vim.lsp.protocol.CompletionItemKind.Method,
  field = vim.lsp.protocol.CompletionItemKind.Field,
  keyword = vim.lsp.protocol.CompletionItemKind.Keyword,
  value = vim.lsp.protocol.CompletionItemKind.Value,
}

function M.to_lsp_item(item)
  local insert_text_format = vim.lsp.protocol.InsertTextFormat.PlainText
  if item.insertTextFormat == "snippet" then
    insert_text_format = vim.lsp.protocol.InsertTextFormat.Snippet
  end
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
    insertTextFormat = insert_text_format,
    preselect = item.metadata and item.metadata.source == "mixin.attribute" or nil,
    data = item.metadata,
  }
end

local function changedtick(bufnr)
  if not vim.api.nvim_buf_is_valid(bufnr) then
    return nil
  end
  return vim.api.nvim_buf_get_changedtick(bufnr)
end

local function document_uri(bufnr)
  return vim.uri_from_bufnr(bufnr)
end

local function cursor_prefix(bufnr, position)
  local line = vim.api.nvim_buf_get_lines(bufnr, position[1] - 1, position[1], false)[1] or ""
  local before = line:sub(1, position[2])
  local prefix = before:match("([%w_.$/;:<>()%-]+)$") or ""
  return prefix, before:sub(1, #before - #prefix)
end

local function item_matches_prefix(item, prefix)
  if prefix == "" then
    return true
  end
  local needle = prefix:lower()
  local fields = {
    item.filterText,
    item.label,
    item.insertText,
    item.detail,
  }
  for _, value in ipairs(fields) do
    if value and tostring(value):lower():find(needle, 1, true) then
      return true
    end
  end
  return false
end

local function filter_items(items, prefix)
  local filtered = {}
  for _, item in ipairs(items or {}) do
    if item_matches_prefix(item, prefix) then
      table.insert(filtered, item)
    end
  end
  return filtered
end

local function cache_key(bufnr, position, source)
  return table.concat({
    document_uri(bufnr),
    tostring(position[1] or ""),
    source or "manual",
  }, "|")
end

local function refresh_cached_edit(item, cached_column, current_column)
  local copy = vim.deepcopy(item)
  local edit = copy.textEdit
  local range = edit and (edit.range or edit.replace)
  if range and range["end"] and range["end"].character and cached_column ~= current_column then
    local suffix_length = math.max(0, range["end"].character - cached_column)
    range["end"].character = current_column + suffix_length
    if edit.insert and edit.insert["end"] then
      edit.insert["end"].character = current_column
    end
  end
  return copy
end

local function local_prefix_cache_hit(key, prefix_base, prefix, current_column)
  if not prefix_cache or prefix_cache.expires_at < vim.loop.hrtime() then
    return nil
  end
  if prefix_cache.key ~= key then
    return nil
  end
  if prefix_cache.prefix_base ~= prefix_base then
    return nil
  end
  if prefix:sub(1, #prefix_cache.prefix) ~= prefix_cache.prefix then
    return nil
  end
  local filtered = filter_items(prefix_cache.items, prefix)
  local refreshed = {}
  for _, item in ipairs(filtered) do
    table.insert(refreshed, refresh_cached_edit(item, prefix_cache.column, current_column))
  end
  return refreshed
end

function M.complete(callback, bufnr, position, opts)
  opts = opts or {}
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local request_tick = changedtick(bufnr)
  local request_prefix, request_prefix_base = cursor_prefix(bufnr, position)
  M.request_seq = M.request_seq + 1
  local request_id = M.request_seq
  M.request_count = M.request_count + 1
  M.last_source = opts.source or "manual"
  local key = cache_key(bufnr, position, M.last_source)
  M.last_request = {
    bufnr = bufnr,
    position = position,
    changedtick = request_tick,
    request_id = request_id,
    prefix = request_prefix,
    prefix_base = request_prefix_base,
    source = M.last_source,
  }
  M.last_error = nil
  M.last_local_prefix_cache_hit = false
  M.last_local_prefix_cache_items = 0

  local previous = active_requests[bufnr]
  if previous then
    previous({ isIncomplete = false, items = {} })
  end

  local completed = false
  local function finish(result)
    if completed then
      return
    end
    completed = true
    if active_requests[bufnr] == finish then
      active_requests[bufnr] = nil
    end
    callback(result)
  end

  local cached_items = local_prefix_cache_hit(key, request_prefix_base, request_prefix, position[2])
  if cached_items then
    M.last_local_prefix_cache_hit = true
    M.last_local_prefix_cache_items = #cached_items
    M.last_callback_item_count = #cached_items
    finish({ isIncomplete = true, items = cached_items })
    return
  end
  active_requests[bufnr] = finish
  M.server_request_count = M.server_request_count + 1
  protocol.completion(function(envelope, err)
    if active_requests[bufnr] ~= finish then
      M.stale_dropped_count = M.stale_dropped_count + 1
      finish({ isIncomplete = false, items = {} })
      return
    end
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      M.last_error = tostring(unwrap_err)
      M.last_callback_item_count = 0
      finish({ isIncomplete = false, items = {} })
      return
    end
    if request_tick ~= changedtick(bufnr) then
      M.stale_dropped_count = M.stale_dropped_count + 1
      M.last_callback_item_count = 0
      finish({ isIncomplete = false, items = {} })
      return
    end
    result = result or { items = {} }
    M.last_debug = result.debug
    if M.last_debug then
      M.last_debug.staleDropped = M.stale_dropped_count
      M.last_debug.localPrefixCacheHit = M.last_local_prefix_cache_hit
      M.last_debug.localPrefixCacheItems = M.last_local_prefix_cache_items
    end
    local items = {}
    for _, item in ipairs(result.items or {}) do
      table.insert(items, M.to_lsp_item(item))
    end
    prefix_cache = {
      key = key,
      prefix_base = request_prefix_base,
      prefix = request_prefix,
      column = position[2],
      items = items,
      expires_at = vim.loop.hrtime() + 2000000000,
    }
    M.last_response_count = #items
    M.last_callback_item_count = #items
    finish({ isIncomplete = result.isIncomplete or false, items = items })
  end, bufnr, position)
end

return M
