local completion = require("mcdev.completion")

local source = {}

function source.new()
  return setmetatable({}, { __index = source })
end

function source:is_available()
  local ft = vim.bo.filetype
  return ft == "java" or ft == "json" or ft == "accesswidener" or ft == "cfg"
end

function source:complete(_, callback)
  completion.complete(function(result)
    callback(result.items)
  end)
end

return source
