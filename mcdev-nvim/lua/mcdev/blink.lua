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
  return type(source_name) == "string"
    and (source_name:sub(1, 6) == "mixin." or source_name:sub(1, 12) == "mixinextras.")
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

local function utf16_to_byte_edit(bufnr, edit)
  if not edit or not edit.range then
    return
  end
  for _, position in ipairs({ edit.range.start, edit.range["end"] }) do
    local line = vim.api.nvim_buf_get_lines(bufnr, position.line, position.line + 1, false)[1]
    if line then
      position.character = vim.str_byteindex(line, "utf-16", position.character, false)
    end
  end
end

local function to_blink_item(item, bufnr, position)
  local blink_item = vim.deepcopy(item)
  blink_item.cursor_column = blink_item._mcdev_cursor_column or position[2] or 0
  blink_item._mcdev_cursor_column = nil
  utf16_to_byte_edit(bufnr, blink_item.textEdit)
  for _, edit in ipairs(blink_item.additionalTextEdits or {}) do
    utf16_to_byte_edit(bufnr, edit)
  end

  if not is_mixin_item(item) then
    return blink_item
  end

  blink_item.kind_icon = mixin_icon
  blink_item.kind_name = "Mixin"

  if item_source(blink_item) == "mixin.attribute" then
    -- Attribute snippets must outrank JDT LS' incomplete `name = ` items.
    blink_item.score_offset = math.max(blink_item.score_offset or 0, 100)
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

local function is_owned_context(bufnr, position)
  return buffer.is_mcdev_completion_context_at(bufnr, position)
end

function source.route_sources(default_sources)
  local fallback = vim.deepcopy(default_sources or {})
  return function()
    local bufnr = vim.api.nvim_get_current_buf()
    local position = vim.api.nvim_win_get_cursor(0)
    if is_owned_context(bufnr, position) then
      return { "lsp", "mcdev" }
    end

    local sources = {}
    for _, source_name in ipairs(fallback) do
      if source_name ~= "mcdev" then
        sources[#sources + 1] = source_name
      end
    end
    return sources
  end
end

function source:enabled(ctx)
  if ctx and not ctx.cursor and not ctx.line_number and not (ctx.line and ctx.col) then
    return buffer.is_mcdev_completion_context(ctx_bufnr(ctx))
  end
  return is_owned_context(ctx_bufnr(ctx), ctx_position(ctx))
end

function source:get_completions(ctx, callback)
  local bufnr = ctx_bufnr(ctx)
  local position = ctx_position(ctx)
  if not is_owned_context(bufnr, position) then
    callback({
      is_incomplete_forward = false,
      is_incomplete_backward = false,
      items = {},
    })
    return
  end
  local seen = {}
  return completion.complete(function(result)
    if result.isStale then
      callback({
        is_incomplete_forward = true,
        is_incomplete_backward = true,
        items = {},
      })
      return
    end
    local items = {}
    for _, item in ipairs(result.items or {}) do
      local key = completion.item_key(item)
      if not seen[key] then
        seen[key] = true
        items[#items + 1] = item
      end
    end
    callback({
      is_incomplete_forward = result.isProvisional or result.isIncomplete or false,
      -- The server filters by the current prefix. Blink must refetch after
      -- backspacing so a result for a longer prefix cannot hide new items.
      is_incomplete_backward = true,
      items = to_blink_items(items, bufnr, position),
    })
  end, bufnr, position, { source = "blink", stream = true })
end

return source
