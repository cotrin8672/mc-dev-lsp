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

local function visible_cursor_position(bufnr)
  local current_winid = vim.api.nvim_get_current_win()
  if vim.api.nvim_win_get_buf(current_winid) == bufnr then
    return vim.api.nvim_win_get_cursor(current_winid)
  end
  for _, winid in ipairs(vim.api.nvim_list_wins()) do
    if vim.api.nvim_win_get_buf(winid) == bufnr then
      return vim.api.nvim_win_get_cursor(winid)
    end
  end
  return nil
end

local function changed_only_by_prefix_extension(bufnr, request_lines, request_position, current_position, extension)
  local request_row = request_position[1]
  local request_column = request_position[2]
  local request_line = request_lines[request_row] or ""
  local expected_lines = vim.deepcopy(request_lines)
  expected_lines[request_row] = request_line:sub(1, request_column)
    .. extension
    .. request_line:sub(request_column + 1)
  local current_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  return current_position[2] == request_column + #extension and vim.deep_equal(current_lines, expected_lines)
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
  if copy._mcdev_cursor_column then
    copy._mcdev_cursor_column = current_column
  end
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
  local request_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
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
      return
    end
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      M.last_error = tostring(unwrap_err)
      M.last_callback_item_count = 0
      finish({ isIncomplete = false, items = {} })
      return
    end
    local response_position = position
    local response_prefix = request_prefix
    local response_prefix_base = request_prefix_base
    local prefix_was_extended = false
    if request_tick ~= changedtick(bufnr) then
      local current_position = visible_cursor_position(bufnr)
      if current_position and current_position[1] == position[1] then
        local current_prefix, current_prefix_base = cursor_prefix(bufnr, current_position)
        local extension = current_prefix:sub(#request_prefix + 1)
        prefix_was_extended = current_prefix_base == request_prefix_base
          and current_prefix:sub(1, #request_prefix) == request_prefix
          and changed_only_by_prefix_extension(bufnr, request_lines, position, current_position, extension)
        if prefix_was_extended then
          response_position = current_position
          response_prefix = current_prefix
          response_prefix_base = current_prefix_base
        end
      end
      if not prefix_was_extended then
        M.stale_dropped_count = M.stale_dropped_count + 1
        M.last_callback_item_count = 0
        -- Blink must retry for the prefix typed while this request was running.
        -- Marking the empty stale result complete makes Blink cache it and hides
        -- every mcdev candidate until completion is manually restarted.
        finish({ isIncomplete = true, items = {} })
        return
      end
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
      local converted = M.to_lsp_item(item)
      if prefix_was_extended then
        converted = refresh_cached_edit(converted, position[2], response_position[2])
        -- Blink compensates edits for cursor movement after a request.  Tell
        -- the adapter that this edit has already been refreshed to the current
        -- column so the movement is not applied a second time.
        converted._mcdev_cursor_column = response_position[2]
      end
      table.insert(items, converted)
    end
    prefix_cache = {
      key = key,
      prefix_base = response_prefix_base,
      prefix = response_prefix,
      column = response_position[2],
      items = items,
      expires_at = vim.loop.hrtime() + 2000000000,
    }
    local response_items = prefix_was_extended and filter_items(items, response_prefix) or items
    M.last_response_count = #response_items
    M.last_callback_item_count = #response_items
    finish({ isIncomplete = result.isIncomplete or false, items = response_items })
  end, bufnr, position)
end

return M
