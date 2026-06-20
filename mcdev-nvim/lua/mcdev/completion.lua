local protocol = require("mcdev.protocol")

local M = {}
M.last_request = nil
M.last_response_count = nil
M.last_error = nil
M.last_debug = nil
M.request_count = 0
M.request_seq = 0
M.stale_dropped_count = 0
M.last_source = nil
M.last_callback_item_count = nil
M.last_local_prefix_cache_hit = false
M.last_local_prefix_cache_items = 0

local prefix_cache = nil

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

local function local_prefix_cache_hit(key, prefix_base, prefix)
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
  return filter_items(prefix_cache.items, prefix)
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
  local cached_items = local_prefix_cache_hit(key, request_prefix_base, request_prefix)
  if cached_items then
    M.last_local_prefix_cache_hit = true
    M.last_local_prefix_cache_items = #cached_items
    M.last_callback_item_count = #cached_items
    callback({ isIncomplete = true, items = cached_items })
  end
  protocol.completion(function(envelope, err)
    if request_id ~= M.request_seq then
      M.stale_dropped_count = M.stale_dropped_count + 1
      return
    end
    if err then
      M.last_error = tostring(err)
      M.last_callback_item_count = 0
      callback({ isIncomplete = false, items = {} })
      return
    end
    if request_tick ~= changedtick(bufnr) then
      M.stale_dropped_count = M.stale_dropped_count + 1
      M.last_callback_item_count = 0
      return
    end
    local result = envelope and envelope.result or { items = {} }
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
      items = items,
      expires_at = vim.loop.hrtime() + 2000000000,
    }
    M.last_response_count = #items
    M.last_callback_item_count = #items
    callback({ isIncomplete = result.isIncomplete or false, items = items })
  end, bufnr, position)
end

return M
