local config = require("mcdev.config")
local buffer = require("mcdev.buffer")
local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")

local M = {}

M.namespace = vim.api.nvim_create_namespace("mcdev")

local debounce_timers = {}

function M.publish(bufnr, diagnostics)
  local vim_diagnostics = {}
  for _, diagnostic in ipairs(diagnostics or {}) do
    table.insert(vim_diagnostics, convert.to_vim_diagnostic(diagnostic))
  end
  vim.diagnostic.set(M.namespace, bufnr, vim_diagnostics)
end

function M.fetch(bufnr, position, cb)
  protocol.diagnostics(bufnr, position, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    if cb then
      cb((result and result.diagnostics) or {}, nil)
    end
  end)
end

function M.refresh(bufnr)
  M.fetch(bufnr, nil, function(diagnostics, err)
    if err then
      return
    end
    M.publish(bufnr, diagnostics)
  end)
end

function M.setup_autocmds()
  local debounce_ms = config.options.diagnostics.debounce_ms or 300
  vim.api.nvim_create_autocmd({ "BufEnter", "TextChanged", "TextChangedI" }, {
    callback = function(args)
      local bufnr = args.buf
      if not config.options.diagnostics.enable then
        return
      end
      if not buffer.is_mcdev_buffer(bufnr) then
        return
      end
      if debounce_timers[bufnr] then
        vim.fn.timer_stop(debounce_timers[bufnr])
      end
      debounce_timers[bufnr] = vim.fn.timer_start(debounce_ms, function()
        debounce_timers[bufnr] = nil
        if vim.api.nvim_buf_is_valid(bufnr) then
          M.refresh(bufnr)
        end
      end)
    end,
  })
end

return M
