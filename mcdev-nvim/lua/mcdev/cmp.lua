local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}

function source.source(opts)
  return setmetatable({ opts = opts or {} }, { __index = source })
end

source.new = source.source

local function cursor_position(cursor)
  if cursor and cursor.row and cursor.col then
    return { cursor.row, cursor.col - 1 }
  end
  return cursor or vim.api.nvim_win_get_cursor(0)
end

function source:is_available()
  return buffer.is_mcdev_completion_context(0)
end

function source:complete(params, callback)
  local context = params and params.context or {}
  local bufnr = context.bufnr or vim.api.nvim_get_current_buf()
  local cursor = cursor_position(context.cursor)
  if not buffer.is_mcdev_completion_context(bufnr) then
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
  end, bufnr, cursor, { source = "cmp", stream = true })
end

return source
