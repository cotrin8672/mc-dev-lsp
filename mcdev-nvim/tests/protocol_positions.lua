local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")

local original_bufnr = vim.api.nvim_get_current_buf()
local original_cursor = vim.api.nvim_win_get_cursor(0)
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname() .. ".java")
vim.api.nvim_set_current_buf(bufnr)

for _, case in ipairs({
  { line = "abc", character = 3 },
  { line = "日@At", character = 4 },
  { line = "😀a", character = 3 },
}) do
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { case.line })
  local context = protocol.context(bufnr, { 1, #case.line })
  helpers.assert_eq(context.position.line, 0)
  helpers.assert_eq(context.position.character, case.character)
end

local line = "日@Atx"
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { line })
vim.api.nvim_win_set_cursor(0, { 1, #line })
local omitted = protocol.build_code_action_payload(bufnr)
helpers.assert_eq(omitted.context.position.character, 4)
helpers.assert_eq(omitted.range.start.character, 4)
helpers.assert_eq(omitted.range["end"].character, 4)

vim.api.nvim_win_set_cursor(0, { 1, 0 })
local explicit_range = {
  start = { line = 0, character = 4 },
  ["end"] = { line = 0, character = 5 },
}
local explicit = protocol.build_code_action_payload(bufnr, explicit_range)
helpers.assert_eq(explicit.context.position.line, explicit_range.start.line)
helpers.assert_eq(explicit.context.position.character, explicit_range.start.character)
helpers.assert_eq(explicit.range.start.character, 4)
helpers.assert_eq(explicit.range["end"].character, 5)

vim.api.nvim_set_current_buf(original_bufnr)
vim.api.nvim_win_set_cursor(0, original_cursor)
vim.api.nvim_buf_delete(bufnr, { force = true })

print("mcdev-nvim protocol position tests passed")
