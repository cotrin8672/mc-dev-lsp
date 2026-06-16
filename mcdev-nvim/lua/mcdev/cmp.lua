local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}

function source.source(opts)
  return setmetatable({ opts = opts or {} }, { __index = source })
end

source.new = source.source

function source:is_available()
  return buffer.is_mcdev_buffer(0)
end

function source:complete(params, callback)
  local context = params and params.context or {}
  local bufnr = context.bufnr or vim.api.nvim_get_current_buf()
  local cursor = context.cursor or vim.api.nvim_win_get_cursor(0)
  completion.complete(function(result)
    callback({
      items = result.items or {},
      isIncomplete = result.isIncomplete or false,
    })
  end, bufnr, cursor)
end

return source
