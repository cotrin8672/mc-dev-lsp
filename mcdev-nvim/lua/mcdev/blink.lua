local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}
source.__index = source

local mixin_icon = ""

local function item_source(item)
  local data = item and (item.data or item.metadata)
  return data and data.source or nil
end

local function is_mixin_item(item)
  local source_name = item_source(item)
  return type(source_name) == "string" and source_name:sub(1, 6) == "mixin."
end

local function is_method_kind(kind)
  return kind == "method" or kind == vim.lsp.protocol.CompletionItemKind.Method
end

local function current_prefix_range(bufnr, position)
  local row = position[1] or 1
  local col = position[2] or 0
  local line = vim.api.nvim_buf_get_lines(bufnr, row - 1, row, false)[1] or ""
  local before = line:sub(1, col)
  local prefix = before:match("([%w_.$/;:<>()%-]+)$") or ""
  return {
    start = { line = row - 1, character = col - #prefix },
    ["end"] = { line = row - 1, character = col },
  }
end

local function to_blink_item(item, bufnr, position)
  if not is_mixin_item(item) then
    return item
  end

  local blink_item = vim.deepcopy(item)
  blink_item.kind_icon = mixin_icon
  blink_item.kind_name = "Mixin"

  if is_method_kind(blink_item.kind) then
    blink_item.kind = vim.lsp.protocol.CompletionItemKind.Value
  end

  if not blink_item.textEdit and blink_item.insertText then
    blink_item.textEdit = {
      range = current_prefix_range(bufnr, position),
      newText = blink_item.insertText,
    }
  end

  return blink_item
end

local function to_blink_items(items, bufnr, position)
  local blink_items = {}
  for _, item in ipairs(items or {}) do
    table.insert(blink_items, to_blink_item(item, bufnr, position))
  end
  return blink_items
end

local function ctx_bufnr(ctx)
  return ctx and ctx.bufnr or vim.api.nvim_get_current_buf()
end

local function ctx_position(ctx)
  if ctx and type(ctx.cursor) == "table" then
    return { ctx.cursor[1] or ctx.cursor.line or 1, ctx.cursor[2] or ctx.cursor.col or 0 }
  end
  if ctx and ctx.line_number then
    return { ctx.line_number, ctx.column or ctx.col or 0 }
  end
  if ctx and ctx.line and ctx.col then
    return { ctx.line, ctx.col }
  end
  return vim.api.nvim_win_get_cursor(0)
end

function source.new(opts)
  return setmetatable({ opts = opts or {} }, source)
end

function source.source(opts)
  return source.new(opts)
end

function source:get_trigger_characters()
  return { '"', "/", ".", ":", "@" }
end

function source:enabled(ctx)
  return buffer.is_mcdev_buffer(ctx_bufnr(ctx))
end

function source:get_completions(ctx, callback)
  local bufnr = ctx_bufnr(ctx)
  local position = ctx_position(ctx)
  completion.complete(function(result)
    callback({
      is_incomplete_forward = result.isIncomplete or false,
      is_incomplete_backward = result.isIncomplete or false,
      items = to_blink_items(result.items, bufnr, position),
    })
  end, bufnr, position, { source = "blink" })
end

return source
