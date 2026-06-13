local completion = require("mcdev.completion")

local M = {}

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
  completion.complete(function(result)
    for _, item in ipairs(result.items or {}) do
      table.insert(items, item.label or item.insertText or "")
    end
    done = true
  end)
  vim.wait(5000, function()
    return done
  end, 50)
  return items
end

return M
