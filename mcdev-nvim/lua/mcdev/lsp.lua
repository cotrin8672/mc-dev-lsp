local code_action = require("mcdev.code_action")
local hover = require("mcdev.hover")
local navigation = require("mcdev.navigation")

local M = {}

local function has_result(result)
  if result == nil then
    return false
  end
  if vim.tbl_islist(result) then
    return #result > 0
  end
  return true
end

local function text_document_params(bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  return {
    textDocument = vim.lsp.util.make_text_document_params(bufnr),
    position = {
      line = position[1] - 1,
      character = position[2],
    },
  }
end

local function request_first(bufnr, method, params, on_result, on_empty)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  vim.lsp.buf_request(bufnr, method, params, function(err, result)
    if not err and has_result(result) then
      on_result(result)
      return
    end
    on_empty()
  end)
end

function M.definition(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  request_first(
    bufnr,
    "textDocument/definition",
    text_document_params(bufnr, position),
    function(result)
      if cb then cb(vim.tbl_islist(result) and result or { result }, nil, result) end
    end,
    function()
      navigation.definition(bufnr, position, cb)
    end
  )
end

function M.references(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local params = text_document_params(bufnr, position)
  params.context = { includeDeclaration = true }
  request_first(
    bufnr,
    "textDocument/references",
    params,
    function(result)
      if cb then cb(result, nil) end
    end,
    function()
      navigation.references(bufnr, position, cb)
    end
  )
end

function M.hover(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  request_first(
    bufnr,
    "textDocument/hover",
    text_document_params(bufnr, position),
    function(result)
      if cb then cb(result, nil) end
    end,
    function()
      hover.hover(bufnr, position, cb)
    end
  )
end

function M.code_actions(bufnr, range, diagnostic_codes, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local resolved_range = range or {
    start = { line = 0, character = 0 },
    ["end"] = { line = 0, character = 0 },
  }
  local params = {
    textDocument = vim.lsp.util.make_text_document_params(bufnr),
    range = resolved_range,
    context = { diagnostics = vim.diagnostic.get(bufnr) },
  }
  request_first(
    bufnr,
    "textDocument/codeAction",
    params,
    function(result)
      if cb then cb(result, nil) end
    end,
    function()
      code_action.code_actions(bufnr, range, diagnostic_codes, cb)
    end
  )
end

return M
