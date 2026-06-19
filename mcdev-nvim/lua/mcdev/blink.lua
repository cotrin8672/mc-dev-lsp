local completion = require("mcdev.completion")
local buffer = require("mcdev.buffer")

local source = {}
source.__index = source

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
  completion.complete(function(result)
    callback({
      is_incomplete_forward = result.isIncomplete or false,
      is_incomplete_backward = result.isIncomplete or false,
      items = result.items or {},
    })
  end, bufnr, ctx_position(ctx), { source = "blink" })
end

return source
