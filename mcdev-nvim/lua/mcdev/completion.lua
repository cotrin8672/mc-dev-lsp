local protocol = require("mcdev.protocol")
local convert = require("mcdev.convert")
local stdio = require("mcdev.stdio")
local transport = require("mcdev.transport")

local M = {}
M.last_request = nil
M.last_response_count = nil
M.last_error = nil
M.last_debug = nil
M.request_count = 0
M.server_request_count = 0
M.request_seq = 0
M.stale_dropped_count = 0
M.cancelled_count = 0
M.last_source = nil
M.last_callback_item_count = nil
M.helper_request_count = 0
M.last_project_transport_error = nil

local active_requests = {}

local function cancel_operation(operation)
  if not operation or operation.cancelled or operation.completed then
    return
  end
  operation.cancelled = true
  M.cancelled_count = M.cancelled_count + 1
  if operation.cancel_transport then
    operation.cancel_transport()
  end
end

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

local function cursor_prefix(bufnr, position)
  local line = vim.api.nvim_buf_get_lines(bufnr, position[1] - 1, position[1], false)[1] or ""
  local before = line:sub(1, position[2])
  local prefix = before:match("([%w_.$/;:<>()%-]+)$") or ""
  return prefix, before:sub(1, #before - #prefix)
end

local function item_key(item)
  local text_edit = item.textEdit or {}
  local range = text_edit.range or {}
  local start = range.start or {}
  local finish = range["end"] or {}
  return table.concat({
    tostring(item.label or ""),
    tostring(item.insertText or text_edit.newText or ""),
    tostring(start.line or ""),
    tostring(start.character or ""),
    tostring(finish.line or ""),
    tostring(finish.character or ""),
  }, "\31")
end

function M.item_key(item)
  return item_key(item)
end

local function merge_items(first, second)
  local merged = {}
  local seen = {}
  for _, items in ipairs({ first or {}, second or {} }) do
    for _, item in ipairs(items) do
      local key = item_key(item)
      if not seen[key] then
        seen[key] = true
        merged[#merged + 1] = item
      end
    end
  end
  return merged
end

local function decode_completion(envelope, err)
  local result, unwrap_err = convert.unwrap_envelope(envelope, err)
  if unwrap_err then
    return nil, unwrap_err
  end
  if result == nil then
    result = { items = {} }
  elseif type(result) ~= "table" then
    return nil, "mcdev: invalid completion response"
  end
  local raw_items = result.items or {}
  if type(raw_items) ~= "table" then
    return nil, "mcdev: invalid completion items"
  end
  local items = {}
  for _, item in ipairs(raw_items) do
    if type(item) ~= "table" then
      return nil, "mcdev: invalid completion item"
    end
    items[#items + 1] = M.to_lsp_item(item)
  end
  return {
    items = items,
    isIncomplete = result.isIncomplete or false,
    debug = result.debug,
  }
end

function M.complete(callback, bufnr, position, opts)
  opts = opts or {}
  callback = callback or function() end
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local request_tick = changedtick(bufnr)
  local request_prefix, request_prefix_base = cursor_prefix(bufnr, position)
  M.request_seq = M.request_seq + 1
  local request_id = M.request_seq
  M.request_count = M.request_count + 1
  M.last_source = opts.source or "manual"
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
  M.last_project_transport_error = nil

  local operation = {
    bufnr = bufnr,
    cancelled = false,
    completed = false,
    cancel_transport = nil,
    cancel_project = nil,
    cancel_helper = nil,
    cancel_jdt = nil,
  }

  local helper_items = {}
  local helper_stream = opts.stream == true
  local helper_done = false
  local jdt_failed = false

  local function cancel_ready_wait()
    if operation.cancel_ready then operation.cancel_ready(); operation.cancel_ready = nil end
    if operation.ready_timer then
      if not operation.ready_timer:is_closing() then
        operation.ready_timer:stop()
        operation.ready_timer:close()
      end
      operation.ready_timer = nil
    end
  end

  local function cancel_auxiliary_requests()
    cancel_ready_wait()
    if type(operation.cancel_project) == "function" then
      operation.cancel_project()
      operation.cancel_project = nil
    end
    if type(operation.cancel_helper) == "function" then
      operation.cancel_helper()
      operation.cancel_helper = nil
    end
    if type(operation.cancel_jdt) == "function" then
      operation.cancel_jdt()
      operation.cancel_jdt = nil
    end
  end

  local function finish(result)
    if operation.completed then
      return
    end
    operation.completed = true
    cancel_ready_wait()
    if active_requests[bufnr] == operation then
      active_requests[bufnr] = nil
    end
    callback(result)
  end

  local function finish_stale()
    if operation.completed then
      return
    end
    M.stale_dropped_count = M.stale_dropped_count + 1
    M.last_callback_item_count = 0
    cancel_auxiliary_requests()
    finish({ isStale = true })
  end

  local function request_is_stale()
    return operation.cancelled or active_requests[bufnr] ~= operation
      or request_tick ~= changedtick(bufnr)
  end

  cancel_operation(active_requests[bufnr])
  active_requests[bufnr] = operation

  operation.cancel_transport = function()
    cancel_auxiliary_requests()
  end

  local function cancel()
    cancel_operation(operation)
  end

  local function finish_helper_fallback()
    if operation.completed or request_is_stale() then
      return
    end
    M.last_response_count = #helper_items
    M.last_callback_item_count = #helper_items
    cancel_auxiliary_requests()
    finish({ isIncomplete = true, items = vim.deepcopy(helper_items) })
  end

  local function on_helper(envelope, err)
    if operation.cancelled or active_requests[bufnr] ~= operation or operation.completed then
      M.stale_dropped_count = M.stale_dropped_count + 1
      return
    end
    if request_tick ~= changedtick(bufnr) then
      finish_stale()
      return
    end

    local result, helper_err = decode_completion(envelope, err)
    helper_done = true
    if helper_err then
      if jdt_failed then
        finish_helper_fallback()
      end
      return
    end
    helper_items = result.items
    if helper_stream and #helper_items > 0 then
      M.last_callback_item_count = #helper_items
      callback({
        isProvisional = true,
        isIncomplete = true,
        items = vim.deepcopy(helper_items),
      })
    end
    if jdt_failed and not request_is_stale() then
      finish_helper_fallback()
    end
  end

  local payload = protocol.build_completion_payload(bufnr, position)
  M.helper_request_count = M.helper_request_count + 1
  local cancel_helper, helper_start_error = stdio.request(payload, on_helper)
  if request_is_stale() or operation.completed then
    if type(cancel_helper) == "function" then
      cancel_helper()
    end
    return cancel
  end
  operation.cancel_helper = cancel_helper
  if helper_start_error and not helper_done then
    helper_done = true
  end

  if request_is_stale() or operation.completed then
    return cancel
  end

  local wait_for_project
  local function on_jdt(envelope, err)
    if operation.cancelled or active_requests[bufnr] ~= operation or operation.completed then
      M.stale_dropped_count = M.stale_dropped_count + 1
      return
    end
    if request_tick ~= changedtick(bufnr) then
      finish_stale()
      return
    end

    if not err and type(envelope) == "table" and type(envelope.error) == "table"
      and type(envelope.error.code) == "string"
      and envelope.error.code:upper() == "INCOMPLETE_PROJECT_CONTEXT" and transport.when_ready then
      -- Import readiness is temporary. Finalizing the helper's empty subset
      -- here cancels the listener that could deliver real target members.
      if wait_for_project() then
        if helper_stream then
          callback({ isProvisional = true, isIncomplete = true, items = vim.deepcopy(helper_items) })
        end
        return
      end
    end
    local result, unwrap_err = decode_completion(envelope, err)
    if unwrap_err then
      jdt_failed = true
      M.last_error = tostring(unwrap_err)
      if helper_done then
        finish_helper_fallback()
      end
      return
    end

    M.last_error = nil
    M.last_debug = result.debug
    local items = merge_items(result.items, helper_items)
    M.last_response_count = #items
    M.last_callback_item_count = #items
    if type(operation.cancel_helper) == "function" then
      operation.cancel_helper()
      operation.cancel_helper = nil
    end
    finish({ isIncomplete = result.isIncomplete or false, items = items })
  end

  if request_is_stale() or operation.completed then
    return cancel
  end
  local jdt_started = false
  local function start_jdt_request()
    if jdt_started or request_is_stale() or operation.completed then
      return
    end
    jdt_started = true
    M.server_request_count = M.server_request_count + 1
    local cancel_jdt = protocol.completion(on_jdt, bufnr, position)
    if request_is_stale() or operation.completed then
      if type(cancel_jdt) == "function" then
        cancel_jdt()
      end
    else
      operation.cancel_jdt = cancel_jdt
    end
  end

  local request_project
  wait_for_project = function()
    if operation.cancel_ready then return true end
    if not transport.when_ready then return false end
    operation.cancel_ready = transport.when_ready(bufnr, function()
      operation.cancel_ready = nil
      if request_is_stale() or operation.completed then return end
      request_project()
    end, operation.project_generation)
    if not operation.cancel_ready then return false end
    if not operation.ready_timer then
      operation.ready_timer = vim.defer_fn(function()
        if request_is_stale() or operation.completed then return end
        M.last_error = "mcdev: project dependencies did not become ready within 120 seconds"
        finish_helper_fallback()
      end, 120000)
    end
    return true
  end

  local function on_project(envelope, err)
    operation.cancel_project = nil
    if operation.cancelled or active_requests[bufnr] ~= operation or operation.completed then
      M.stale_dropped_count = M.stale_dropped_count + 1
      return
    end
    if request_tick ~= changedtick(bufnr) then
      finish_stale()
      return
    end
    local decoded, decode_error = decode_completion(envelope, err)
    if not decoded or type(envelope) ~= "table" or type(envelope.result) ~= "table" then
      M.last_project_transport_error = decode_error or "invalid transport envelope"
      wait_for_project()
      start_jdt_request()
      return
    end
    if operation.cancel_jdt then operation.cancel_jdt(); operation.cancel_jdt = nil end
    M.last_project_transport_error = nil
    on_jdt(envelope, nil)
  end

  request_project = function()
    operation.project_generation = transport.ready_generation and transport.ready_generation(bufnr) or nil
    local cancel_project, project_start_error = transport.request(payload, on_project, bufnr)
    if cancel_project then
      if operation.completed or operation.cancelled then cancel_project()
      else operation.cancel_project = cancel_project end
    else
      M.last_project_transport_error = project_start_error
      wait_for_project()
      start_jdt_request()
    end
  end
  request_project()
  return cancel
end

return M
