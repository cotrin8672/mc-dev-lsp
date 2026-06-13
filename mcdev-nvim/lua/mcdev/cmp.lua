local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}

function source.new()
  return setmetatable({}, { __index = source })
end

function source:is_available()
  return buffer.is_mcdev_buffer(0)
end

function source:complete(_, callback)
  completion.complete(function(result)
    callback(result.items)
  end)
end

return source
