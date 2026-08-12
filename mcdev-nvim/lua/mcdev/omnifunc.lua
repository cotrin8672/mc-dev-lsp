local completion = require("mcdev.completion")
local config = require("mcdev.config")

local M = {}
M.last_timeout = false

function M.complete(findstart, base)
  if findstart == 1 then
    local line = vim.api.nvim_get_current_line()
    local col = vim.api.nvim_win_get_cursor(0)[2]
    local start = col
    while start > 0 and line:sub(start, start):match("[%w_$]") do
      start = start - 1
    end
    return start
  end

  local items = {}
  local done = false
  M.last_timeout = false
  completion.complete(function(result)
    for _, item in ipairs(result.items or {}) do
      if item.insertTextFormat ~= vim.lsp.protocol.InsertTextFormat.Snippet then
        table.insert(items, {
          word = item.insertText or item.label or "",
          abbr = item.label or item.insertText or "",
          menu = item.detail or "[mcdev]",
          info = item.documentation or "",
        })
      end
    end
    done = true
  end)
  local completed = vim.wait(config.options.completion.omnifunc_timeout_ms or 500, function()
    return done
  end, 20)
  if not completed then
    M.last_timeout = true
    return {}
  end
  return items
end

return M
