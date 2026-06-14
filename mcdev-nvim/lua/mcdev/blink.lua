local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local M = {}

function M.source()
  return setmetatable({}, { __index = M })
end

M.new = M.source

function M:get_trigger_characters()
  return { '"', "@", "." }
end

function M:enabled()
  return buffer.is_mcdev_buffer(0)
end

function M:get_completions(_, callback)
  completion.complete(function(result)
    callback({
      is_incomplete_forward = result.isIncomplete,
      is_incomplete_backward = false,
      items = result.items,
    })
  end)
end

return M
