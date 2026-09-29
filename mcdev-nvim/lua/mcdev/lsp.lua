local code_action = require("mcdev.code_action")
local hover = require("mcdev.hover")
local navigation = require("mcdev.navigation")
local convert = require("mcdev.convert")

local M = {}

local function has_result(result)
  if result == nil or result == vim.NIL then
    return false
  end
  if type(result) == "string" then return result ~= "" end
  if result.contents ~= nil then return has_result(result.contents) end
  if vim.tbl_islist(result) then
    return #result > 0
  end
  return true
end

local function text_document_params(bufnr, position, context)
  return function(client)
    return {
      textDocument = vim.lsp.util.make_text_document_params(bufnr),
      position = convert.to_lsp_position(bufnr, position, client.offset_encoding),
      context = context,
    }
  end
end

local function request_first(bufnr, method, params, on_result, on_empty)
  local client_count = #vim.lsp.get_clients({ bufnr = bufnr, method = method })
  if client_count == 0 then
    on_empty()
    return
  end
  local remaining, finished = client_count, false
  local function finish_empty()
    if not finished and remaining == 0 then
      finished = true
      on_empty()
    end
  end
  local requests = vim.lsp.buf_request(bufnr, method, params, function(err, result, ctx)
    if finished then return end
    remaining = remaining - 1
    if not err and has_result(result) then
      finished = true
      local client = vim.lsp.get_client_by_id(ctx.client_id)
      on_result(result, client and client.offset_encoding or "utf-16")
    else
      finish_empty()
    end
  end)
  -- A client can stop between discovery and dispatch without invoking its callback.
  remaining = remaining - (client_count - vim.tbl_count(requests))
  finish_empty()
end

function M.definition(bufnr, position, cb)
  bufnr = bufnr and bufnr ~= 0 and bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  request_first(
    bufnr,
    "textDocument/definition",
    text_document_params(bufnr, position),
    function(result, encoding)
      if cb then cb(vim.tbl_islist(result) and result or { result }, nil, result, encoding) end
    end,
    function()
      navigation.definition(bufnr, position, cb)
    end
  )
end

function M.references(bufnr, position, cb)
  bufnr = bufnr and bufnr ~= 0 and bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local params = text_document_params(bufnr, position, { includeDeclaration = true })
  request_first(
    bufnr,
    "textDocument/references",
    params,
    function(result, encoding)
      if cb then cb(result, nil, nil, encoding) end
    end,
    function()
      navigation.references(bufnr, position, cb)
    end
  )
end

function M.hover(bufnr, position, cb)
  bufnr = bufnr and bufnr ~= 0 and bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
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
  bufnr = bufnr and bufnr ~= 0 and bufnr or vim.api.nvim_get_current_buf()
  local cursor = convert.to_lsp_position(bufnr, vim.api.nvim_win_get_cursor(0))
  local resolved_range = range or {
    start = cursor,
    ["end"] = cursor,
  }
  local function params(client)
    local diagnostics = vim.tbl_map(function(diagnostic)
      local converted = vim.deepcopy(diagnostic.user_data and diagnostic.user_data.lsp or {
        message = diagnostic.message, severity = diagnostic.severity,
        code = diagnostic.code, source = diagnostic.source,
      })
      converted.range = {
        start = convert.to_lsp_position(bufnr, { diagnostic.lnum + 1, diagnostic.col }, client.offset_encoding),
        ["end"] = convert.to_lsp_position(bufnr,
          { (diagnostic.end_lnum or diagnostic.lnum) + 1, diagnostic.end_col or diagnostic.col }, client.offset_encoding),
      }
      return converted
    end, vim.diagnostic.get(bufnr))
    return {
      textDocument = vim.lsp.util.make_text_document_params(bufnr),
      range = convert.to_lsp_range(bufnr, resolved_range, client.offset_encoding),
      context = { diagnostics = diagnostics },
    }
  end
  local standard_actions = nil
  local standard_error = nil
  local mcdev_actions = nil
  local mcdev_error = nil

  local function finish()
    if standard_actions == nil or mcdev_actions == nil then
      return
    end
    local merged = {}
    for _, actions in ipairs({ standard_actions, mcdev_actions }) do
      for _, action in ipairs(actions) do
        -- ponytail: small action lists; use a keyed index if these become large.
        if not vim.tbl_contains(merged, function(existing) return vim.deep_equal(existing, action) end, { predicate = true }) then
          merged[#merged + 1] = action
        end
      end
    end
    local err = #merged == 0 and (standard_error or mcdev_error) or nil
    if cb then cb(merged, err) end
  end

  local function collect_standard(results)
    standard_actions = {}
    for client_id, response in pairs(results or {}) do
      local response_error = response.err or response.error
      if response_error and not standard_error then
        standard_error = type(response_error) == "table" and (response_error.message or vim.inspect(response_error))
          or tostring(response_error)
      end
      for _, action in ipairs(response.result ~= vim.NIL and response.result or {}) do
        local item = vim.deepcopy(action)
        item._mcdev_client_id = client_id
        standard_actions[#standard_actions + 1] = item
      end
    end
    finish()
  end

  if #vim.lsp.get_clients({ bufnr = bufnr, method = "textDocument/codeAction" }) > 0 then
    vim.lsp.buf_request_all(bufnr, "textDocument/codeAction", params, collect_standard)
  else
    collect_standard({})
  end

  code_action.code_actions(bufnr, resolved_range, diagnostic_codes, function(actions, err)
    mcdev_actions = actions or {}
    mcdev_error = err
    finish()
  end)
end

return M
