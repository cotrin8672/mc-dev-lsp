local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")

local M = {}

function M.hover(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  protocol.hover(bufnr, position, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    if cb then
      cb(result, nil)
    end
  end)
end

function M.show(bufnr, position)
  M.hover(bufnr, position, function(result, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    local contents = result and result.contents or {}
    if #contents == 0 then
      vim.notify("mcdev: no hover found", vim.log.levels.INFO)
      return
    end
    vim.lsp.util.open_floating_preview(contents, "markdown", { border = "rounded" })
  end)
end

return M
