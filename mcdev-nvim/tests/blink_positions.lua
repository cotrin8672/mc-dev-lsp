local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local completion = require("mcdev.completion")
local blink = require("mcdev.blink")

local adapter = blink.source()
local bufnr = vim.api.nvim_create_buf(false, true)
vim.bo[bufnr].filetype = "java"
local original_complete = completion.complete
local current_line = '@Inject(method = "é😀fo'

local function set_lines(lines)
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  current_line = lines[1]
end

local function complete_with(item)
  local result
  completion.complete = function(callback)
    callback({ isIncomplete = false, items = { item } })
  end
  adapter:get_completions({ bufnr = bufnr, cursor = { 1, #current_line } }, function(value)
    result = value
  end)
  return result.items[1], result
end

set_lines({ current_line, "é😀tail" })
local server_item = {
  label = "bar",
  textEdit = {
    range = {
      start = { line = 0, character = 21 },
      ["end"] = { line = 0, character = 23 },
    },
    newText = "bar",
  },
  additionalTextEdits = {
    {
      range = {
        start = { line = 1, character = 3 },
        ["end"] = { line = 1, character = 7 },
      },
      newText = "done",
    },
  },
  data = { source = "jdt", marker = { kept = true } },
}
local converted, converted_result = complete_with(server_item)
helpers.assert_eq(converted_result.is_incomplete_forward, false)
helpers.assert_true(converted_result.is_incomplete_backward)
helpers.assert_eq(converted.textEdit.range.start.character, 24)
helpers.assert_eq(converted.textEdit.range["end"].character, 26)
helpers.assert_eq(converted.additionalTextEdits[1].range.start.character, 6)
helpers.assert_eq(converted.additionalTextEdits[1].range["end"].character, 10)
helpers.assert_eq(converted.textEdit.newText, "bar")
helpers.assert_eq(converted.additionalTextEdits[1].newText, "done")
helpers.assert_true(converted.data.marker.kept)
helpers.assert_eq(server_item.textEdit.range.start.character, 21)
helpers.assert_eq(server_item.additionalTextEdits[1].range.start.character, 3)
local edits = { converted.textEdit }
vim.list_extend(edits, converted.additionalTextEdits)
vim.lsp.util.apply_text_edits(edits, bufnr, "utf-8")
helpers.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)[1], '@Inject(method = "é😀bar')
helpers.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 1, 2, false)[1], "é😀done")

set_lines({ current_line })
local fallback = complete_with({
  label = "bar",
  insertText = "bar",
  data = { source = "mixin.injectMethod" },
})
helpers.assert_eq(fallback.textEdit.range.start.character, 24)
helpers.assert_eq(fallback.textEdit.range["end"].character, 26)
vim.lsp.util.apply_text_edits({ fallback.textEdit }, bufnr, "utf-8")
helpers.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], '@Inject(method = "é😀bar')

completion.complete = original_complete
vim.api.nvim_buf_delete(bufnr, { force = true })
print("mcdev-nvim blink position tests passed")
