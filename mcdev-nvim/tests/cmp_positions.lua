local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local cmp = require("mcdev.cmp")
local protocol = require("mcdev.protocol")

local previous_bufnr = vim.api.nvim_get_current_buf()
local previous_cursor = vim.api.nvim_win_get_cursor(0)
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, "UnicodeMixin.java")
vim.bo[bufnr].filetype = "java"
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "日@Atx" })
vim.api.nvim_set_current_buf(bufnr)

local original_request = protocol.request
local captured = nil
protocol.request = function(command, payload, callback, request_bufnr)
  captured = { command = command, payload = payload, bufnr = request_bufnr }
  callback({
    result = {
      items = {
        {
          label = "accepted",
          insertText = "accepted",
          kind = "value",
          sortKey = "0000_accepted",
          filterText = "accepted",
          additionalEdits = {},
          metadata = { source = "mixin.test" },
        },
      },
    },
  }, nil)
  return function() end
end

local result = nil
cmp.source():complete({
  context = {
    bufnr = bufnr,
    cursor = { row = 1, col = 7, line = 0, character = 4 },
  },
}, function(items)
  result = items
end)

helpers.assert_eq(captured.command, protocol.commands.completion)
helpers.assert_eq(captured.bufnr, bufnr)
helpers.assert_eq(captured.payload.context.position.line, 0)
helpers.assert_eq(captured.payload.context.position.character, 4)
helpers.assert_not_nil(result)
helpers.assert_eq(#result.items, 1)
helpers.assert_eq(result.items[1].label, "accepted")

protocol.request = original_request
vim.api.nvim_set_current_buf(previous_bufnr)
vim.api.nvim_win_set_cursor(0, previous_cursor)
vim.api.nvim_buf_delete(bufnr, { force = true })

print("mcdev-nvim cmp position tests passed")
