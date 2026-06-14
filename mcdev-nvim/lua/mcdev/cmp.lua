local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}

function source.source()
  return setmetatable({}, { __index = source })
end

source.new = source.source

function source:is_available()
  return buffer.is_mcdev_buffer(0)
end

function source:complete(_, callback)
  completion.complete(function(result)
    callback(result.items)
  end)
end

return source
