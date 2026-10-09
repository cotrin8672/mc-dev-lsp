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

local function complete_with(item, cursor)
  local result
  completion.complete = function(callback)
    callback({ isIncomplete = false, items = { item } })
  end
  adapter:get_completions({ bufnr = bufnr, cursor = cursor or { 1, #current_line } }, function(value)
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

local split_lines = {
  "package p;",
  '@At(target = "éLnet/minecraft/core/BlockPos;setPl" +',
  '    "acedBy(" +',
  '    "tail😀")',
}
set_lines(split_lines)
local split_start_line = split_lines[2]
local split_end_line = split_lines[4]
local split_start_byte = #('@At(target = "é')
local split_end_byte = #('    "tail😀')
local split_item, split_result = complete_with({
  label = "FullTarget",
  textEdit = {
    range = {
      start = {
        line = 1,
        character = vim.str_utfindex(split_start_line, "utf-16", split_start_byte, false),
      },
      ["end"] = {
        line = 3,
        character = vim.str_utfindex(split_end_line, "utf-16", split_end_byte, false),
      },
    },
    newText = "FullTarget",
  },
  additionalTextEdits = {
    {
      range = {
        start = { line = 0, character = #"package p;" },
        ["end"] = { line = 0, character = #"package p;" },
      },
      newText = "\nimport example.Added;",
    },
  },
  data = { source = "mixin.atTarget" },
}, { 3, #split_lines[3] })
helpers.assert_eq(split_result.is_incomplete_forward, false)
helpers.assert_eq(split_item.textEdit.range.start.line, 2)
helpers.assert_eq(split_item.textEdit.range["end"].line, 2)
helpers.assert_eq(split_item.textEdit.range.start.character, 0)
helpers.assert_eq(split_item.textEdit.newText, '@At(target = "éFullTarget')
helpers.assert_eq(split_item.additionalTextEdits[1].range.start.line, 1)
helpers.assert_eq(split_item.additionalTextEdits[1].range.start.character, 0)
helpers.assert_eq(split_item.additionalTextEdits[1].range["end"].line, 2)
helpers.assert_eq(split_item.additionalTextEdits[1].range["end"].character, 0)
local split_edits = { split_item.textEdit }
vim.list_extend(split_edits, split_item.additionalTextEdits)
vim.lsp.util.apply_text_edits(split_edits, bufnr, "utf-8")
helpers.assert_eq(
  table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
  table.concat({ "package p;", "import example.Added;", '@At(target = "éFullTarget")' }, "\n")
)

completion.complete = original_complete
vim.api.nvim_buf_delete(bufnr, { force = true })
print("mcdev-nvim blink position tests passed")
