local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local cmp = require("mcdev.cmp")
local completion = require("mcdev.completion")

local previous_bufnr = vim.api.nvim_get_current_buf()
local previous_cursor = vim.api.nvim_win_get_cursor(0)

local function with_buffer(lines, callback)
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, "/project/src/main/java/com/example/ExampleMixin.java")
  vim.bo[bufnr].filetype = "java"
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_set_current_buf(bufnr)
  callback(bufnr)
  vim.api.nvim_set_current_buf(previous_bufnr)
  vim.api.nvim_win_set_cursor(0, previous_cursor)
  vim.api.nvim_buf_delete(bufnr, { force = true })
end

local function nvim_cursor(line)
  return { row = 1, col = #line + 1 }
end

local original_complete = completion.complete
local calls = {}
completion.complete = function(callback, bufnr, position, opts)
  calls[#calls + 1] = {
    bufnr = bufnr,
    position = position,
    opts = opts,
  }
  callback({
    isIncomplete = false,
    items = {
      { label = "custom", metadata = { source = "at.member" } },
      { label = "mixin", metadata = { source = "mixin.injectMethod" } },
    },
  })
  return function() end
end

with_buffer({ "class Example {}" }, function(bufnr)
  local line = "class Example {}"
  vim.api.nvim_win_set_cursor(0, { 1, #line })
  local source = cmp.source()
  helpers.assert_eq(source:is_available(), false)

  local result = nil
  source:complete({
    context = { bufnr = bufnr, cursor = nvim_cursor(line) },
  }, function(value)
    result = value
  end)
  helpers.assert_not_nil(result)
  helpers.assert_eq(#result.items, 0)
  helpers.assert_eq(#calls, 0)
end)

with_buffer({ '@Inject(method = "ti' }, function(bufnr)
  local line = '@Inject(method = "ti'
  vim.api.nvim_win_set_cursor(0, { 1, #line })
  local source = cmp.source()
  helpers.assert_true(source:is_available())

  local result = nil
  source:complete({
    context = { bufnr = bufnr, cursor = { line = 0, character = #line } },
  }, function(value)
    result = value
  end)
  helpers.assert_not_nil(result)
  helpers.assert_eq(#result.items, 2)
  helpers.assert_eq(calls[#calls].bufnr, bufnr)
  helpers.assert_eq(calls[#calls].position[1], 1)
  helpers.assert_eq(calls[#calls].position[2], #line)
  helpers.assert_eq(calls[#calls].opts.source, "cmp")
  helpers.assert_true(calls[#calls].opts.stream)

  -- Reusing the same source object after moving outside the annotation must
  -- not leave its earlier Mixin candidates visible or issue a new request.
  local request_count = #calls
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class Example {}" })
  vim.api.nvim_win_set_cursor(0, { 1, #"class Example {}" })
  helpers.assert_eq(source:is_available(), false)
  local ordinary_result = nil
  source:complete({
    context = { bufnr = bufnr, cursor = nvim_cursor("class Example {}") },
  }, function(value)
    ordinary_result = value
  end)
  helpers.assert_not_nil(ordinary_result)
  helpers.assert_eq(#ordinary_result.items, 0)
  helpers.assert_eq(#calls, request_count)
end)

-- Java class/type positions remain available to the mcdev source and retain
-- every backend item.  The cmp adapter must not discard valid non-mixin
-- metadata such as Access Transformer/Access Widener entries.
with_buffer({ "@Mixin(Item.cla" }, function(bufnr)
  local line = "@Mixin(Item.cla"
  vim.api.nvim_win_set_cursor(0, { 1, #line })
  local source = cmp.source()
  helpers.assert_true(source:is_available())

  local result = nil
  source:complete({
    context = { bufnr = bufnr, cursor = nvim_cursor(line) },
  }, function(value)
    result = value
  end)
  helpers.assert_not_nil(result)
  helpers.assert_eq(#result.items, 2)
end)

-- An explicit request may target a non-current buffer.  The source must use
-- that request position instead of the current window's ordinary Java code.
local target_bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(target_bufnr, "/project/src/main/java/com/example/TargetMixin.java")
vim.bo[target_bufnr].filetype = "java"
vim.api.nvim_buf_set_lines(target_bufnr, 0, -1, false, { '@Inject(method = "ti' })
local explicit_result = nil
cmp.source():complete({
  context = {
    bufnr = target_bufnr,
    cursor = { line = 0, character = #'@Inject(method = "ti' },
  },
}, function(value)
  explicit_result = value
end)
helpers.assert_not_nil(explicit_result)
helpers.assert_eq(#explicit_result.items, 2)
helpers.assert_eq(calls[#calls].bufnr, target_bufnr)
vim.api.nvim_buf_delete(target_bufnr, { force = true })

-- Source entry filtering is opt-in for nvim-cmp's generic providers.  It
-- composes an existing source filter and keeps all other source options.
with_buffer({ '@Inject(method = "ti' }, function(bufnr)
  local line = '@Inject(method = "ti'
  local original_filter_calls = 0
  local original_filter = function(entry)
    original_filter_calls = original_filter_calls + 1
    return entry.label ~= "drop"
  end
  local source_config = {
    name = "nvim_lsp",
    priority = 1000,
    option = { keyword_pattern = "keep" },
    entry_filter = original_filter,
  }
  local filtered = cmp.with_exclusive_filter(source_config)
  helpers.assert_true(filtered ~= source_config)
  helpers.assert_eq(filtered.name, source_config.name)
  helpers.assert_eq(filtered.priority, source_config.priority)
  helpers.assert_eq(filtered.option, source_config.option)
  helpers.assert_eq(source_config.entry_filter, original_filter)
  helpers.assert_eq(filtered.entry_filter({ label = "keep" }, {
    bufnr = bufnr,
    cursor = nvim_cursor(line),
  }), false)
  helpers.assert_eq(original_filter_calls, 1)

  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class Example {}" })
  helpers.assert_eq(filtered.entry_filter({ label = "keep" }, {
    bufnr = bufnr,
    cursor = nvim_cursor("class Example {}"),
  }), true)
  helpers.assert_eq(filtered.entry_filter({ label = "drop" }, {
    bufnr = bufnr,
    cursor = nvim_cursor("class Example {}"),
  }), false)
  helpers.assert_eq(original_filter_calls, 3)
end)

completion.complete = original_complete
vim.api.nvim_set_current_buf(previous_bufnr)
vim.api.nvim_win_set_cursor(0, previous_cursor)

print("mcdev-nvim cmp routing tests passed")
