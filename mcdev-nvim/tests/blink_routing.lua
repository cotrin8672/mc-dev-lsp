local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local blink = require("mcdev.blink")
local buffer = require("mcdev.buffer")

local original_bufnr = vim.api.nvim_get_current_buf()
local defaults = { "snippets", "lazydev", "copilot", "buffer", "path", "lsp", "mcdev" }
local routed_sources = blink.route_sources(defaults)

local function assert_sources(actual, expected)
  helpers.assert_eq(#actual, #expected)
  for index, source_name in ipairs(expected) do
    helpers.assert_eq(actual[index], source_name)
  end
end

local function with_buffer(lines, callback)
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, "/project/src/main/java/com/example/Example.java")
  vim.bo[bufnr].filetype = "java"
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_set_current_buf(bufnr)
  callback(bufnr)
  vim.api.nvim_set_current_buf(original_bufnr)
  vim.api.nvim_buf_delete(bufnr, { force = true })
end

with_buffer({ '@Inject(method = "foo"' }, function(bufnr)
  local position = { 1, #'@Inject(method = "foo"' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "lsp", "mcdev" })
end)

with_buffer({ "@Inject(method = \"foo\")", "import com.example.World;" }, function(bufnr)
  local position = { 2, #"import com.example.World;" }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false)
  assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
end)

with_buffer({ 'String text = "@Inject";' }, function(bufnr)
  local position = { 1, #'String text = "@Inject";' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false)
  assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
end)

with_buffer({ "@Inject" }, function(bufnr)
  local position = { 1, #"@Inject" }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "lsp", "mcdev" })
end)

with_buffer({ "@Inject(", "method = \"foo\"" }, function(bufnr)
  local position = { 2, #'method = "foo"' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "lsp", "mcdev" })
end)

for _, annotation in ipairs({ "Wrap", "WrapOperation", "Expression", "Definition", "Local", "Share" }) do
  with_buffer({ "@" .. annotation .. "(value = \"foo\"" }, function(bufnr)
    local position = { 1, #("@" .. annotation .. "(value = \"foo\"") }
    helpers.assert_true(buffer.is_mcdev_completion_context(bufnr), annotation)
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), annotation)
  end)
end

with_buffer({ "@Cancellable" }, function(bufnr)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
end)

local completion = require("mcdev.completion")
local original_complete = completion.complete
local source_requests = 0
completion.complete = function(callback)
  source_requests = source_requests + 1
  callback({ items = { { label = "annotation", insertText = "annotation" } } })
end

for _, prefix in ipairs({ "", "Mix", "In", "At", "Wrap", "Expr", "Definition", "Local", "Cancellable" }) do
  with_buffer({ "@" .. prefix .. " " }, function(bufnr)
    local position = { 1, #("@" .. prefix) }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "lsp", "mcdev" })
    local result
    blink.new():get_completions({ bufnr = bufnr, cursor = position }, function(value)
      result = value
    end)
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), prefix)
    helpers.assert_eq(#result.items, 1, prefix)
  end)
end

helpers.assert_eq(source_requests, 9)
completion.complete = original_complete

print("mcdev-nvim blink routing tests passed")
