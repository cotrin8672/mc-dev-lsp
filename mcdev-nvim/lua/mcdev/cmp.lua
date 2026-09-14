local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}

function source.source(opts)
  return setmetatable({ opts = opts or {} }, { __index = source })
end

source.new = source.source

local function cursor_position(bufnr, cursor)
  if type(cursor) ~= "table" then
    return vim.api.nvim_win_get_cursor(0)
  end
  -- nvim-cmp's row/col pair is the authoritative byte position.  Some
  -- callers also carry LSP coordinates; use those only when row/col is absent.
  if cursor.row ~= nil and cursor.col ~= nil then
    return { cursor.row, math.max(cursor.col - 1, 0) }
  end
  if cursor.line ~= nil and cursor.character ~= nil then
    local line = vim.api.nvim_buf_get_lines(bufnr, cursor.line, cursor.line + 1, false)[1] or ""
    return {
      cursor.line + 1,
      vim.str_byteindex(line, "utf-16", cursor.character, false),
    }
  end
  if cursor[1] ~= nil and cursor[2] ~= nil then
    return { cursor[1], cursor[2] }
  end
  return vim.api.nvim_win_get_cursor(0)
end

function source:is_available()
  local bufnr = vim.api.nvim_get_current_buf()
  local position = vim.api.nvim_win_get_cursor(0)
  return buffer.completion_context_at(bufnr, position) ~= nil
end

function source:complete(params, callback)
  local context = params and params.context or {}
  local bufnr = context.bufnr or vim.api.nvim_get_current_buf()
  local cursor = cursor_position(bufnr, context.cursor)
  if not buffer.completion_context_at(bufnr, cursor) then
    callback({ items = {}, isIncomplete = false })
    return
  end
  completion.complete(function(result)
    if result.isStale then
      -- cmp keeps a source request in FETCHING until its callback runs.  A
      -- stale response still has to settle that request so later requests
      -- are not permanently suppressed.
      callback({ items = {}, isIncomplete = true })
      return
    end
    callback({
      items = result.items or {},
      isIncomplete = result.isProvisional or result.isIncomplete or false,
    })
  end, bufnr, cursor, {
    source = "cmp",
    stream = true,
  })
end

local function copy_source(source_config)
  local copy = {}
  for key, value in pairs(source_config) do
    copy[key] = value
  end
  return copy
end

-- nvim-cmp calls entry_filter once per candidate. Cache the semantic lookup
-- until the buffer, position, or text changes instead of rescanning for each
-- item in the same query.
function source.with_exclusive_filter(source_config)
  local filtered = copy_source(source_config)
  local existing_filter = filtered.entry_filter
  local cached
  filtered.entry_filter = function(entry, ctx)
    if existing_filter and not existing_filter(entry, ctx) then
      return false
    end

    local bufnr = ctx and ctx.bufnr or vim.api.nvim_get_current_buf()
    local cursor = cursor_position(bufnr, ctx and ctx.cursor)
    local changedtick = vim.api.nvim_buf_get_changedtick(bufnr)
    if not cached
      or cached.bufnr ~= bufnr
      or cached.row ~= cursor[1]
      or cached.col ~= cursor[2]
      or cached.changedtick ~= changedtick
    then
      local context = buffer.completion_context_at(bufnr, cursor)
      cached = {
        bufnr = bufnr,
        row = cursor[1],
        col = cursor[2],
        changedtick = changedtick,
        exclusive = context ~= nil and context.exclusive == true,
      }
    end
    return not cached.exclusive
  end
  return filtered
end

return source
