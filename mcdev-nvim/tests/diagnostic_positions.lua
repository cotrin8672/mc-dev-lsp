local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local diagnostics = require("mcdev.diagnostics")

local previous_bufnr = vim.api.nvim_get_current_buf()
local target_bufnr = vim.api.nvim_create_buf(false, true)
local current_bufnr = vim.api.nvim_create_buf(false, true)

vim.api.nvim_buf_set_lines(target_bufnr, 0, -1, false, { "Aé😀target" })
vim.api.nvim_buf_set_lines(current_bufnr, 0, -1, false, { "current buffer" })
vim.api.nvim_set_current_buf(current_bufnr)

diagnostics.publish(target_bufnr, {
  {
    code = "UTF16_POSITION",
    severity = "error",
    message = "target",
    range = {
      start = { line = 0, character = 4 },
      ["end"] = { line = 0, character = 10 },
    },
  },
})

local published = vim.diagnostic.get(target_bufnr, { namespace = diagnostics.namespace })
helpers.assert_eq(#published, 1)
helpers.assert_eq(published[1].lnum, 0)
helpers.assert_eq(published[1].col, 7)
helpers.assert_eq(published[1].end_lnum, 0)
helpers.assert_eq(published[1].end_col, 13)

diagnostics.publish(target_bufnr, {
  {
    code = "UTF16_CLAMP",
    severity = "error",
    message = "clamp",
    range = {
      start = { line = 0, character = 4 },
      ["end"] = { line = 0, character = 99 },
    },
  },
})
helpers.assert_eq(vim.diagnostic.get(target_bufnr, { namespace = diagnostics.namespace })[1].end_col, 13)

vim.api.nvim_set_current_buf(previous_bufnr)
vim.api.nvim_buf_delete(target_bufnr, { force = true })
vim.api.nvim_buf_delete(current_bufnr, { force = true })
