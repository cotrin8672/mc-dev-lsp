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

local function compare_positions(left, right)
  if left.line ~= right.line then
    return left.line < right.line and -1 or 1
  end
  if left.character ~= right.character then
    return left.character < right.character and -1 or 1
  end
  return 0
end

local function contains_position(range, position)
  if compare_positions(range.start, range["end"]) == 0 then
    return compare_positions(range.start, position) == 0
  end
  return compare_positions(range.start, position) <= 0
    and compare_positions(position, range["end"]) < 0
end

local function ranges_intersect(left, right)
  local left_point = compare_positions(left.start, left["end"]) == 0
  local right_point = compare_positions(right.start, right["end"]) == 0
  if left_point then return contains_position(right, left.start) end
  if right_point then return contains_position(left, right.start) end
  return compare_positions(left.start, right["end"]) < 0
    and compare_positions(right.start, left["end"]) < 0
end

local jdt_source_aliases = {
  ["source.generate.constructors"] = true,
  ["source.generate.toString"] = true,
  ["source.overrideMethods"] = true,
  ["source.sortMembers"] = true,
}

local function command_identity(action)
  if type(action.command) == "string" then
    return { command = action.command, arguments = action.arguments }
  end
  if type(action.command) ~= "table" or action.command.command == nil then
    return nil
  end
  return { command = action.command.command, arguments = action.command.arguments }
end

local function same_action(existing, action)
  if vim.deep_equal(existing, action) then
    return true
  end
  if existing._mcdev_client_id == nil or existing._mcdev_client_id ~= action._mcdev_client_id then
    return false
  end

  local existing_command, action_command = command_identity(existing), command_identity(action)
  if existing_command and action_command
    and vim.deep_equal(existing_command, action_command)
    and vim.deep_equal(existing.edit, action.edit)
    and vim.deep_equal(existing.disabled, action.disabled) then
    return true
  end

  if existing.title ~= action.title then
    return false
  end
  local existing_client = vim.lsp.get_client_by_id(existing._mcdev_client_id)
  if not existing_client or existing_client.name ~= "jdtls" then
    return false
  end
  local source_kind
  if existing.kind == "quickassist" and jdt_source_aliases[action.kind] then
    source_kind = action.kind
  elseif action.kind == "quickassist" and jdt_source_aliases[existing.kind] then
    source_kind = existing.kind
  end
  if not source_kind then
    return false
  end
  -- JDTLS gives these aliases different resolve data; only merge equal effects.
  return vim.deep_equal(existing.edit, action.edit)
    and vim.deep_equal(command_identity(existing), command_identity(action))
    and vim.deep_equal(existing.diagnostics, action.diagnostics)
    and vim.deep_equal(existing.disabled, action.disabled)
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
    local diagnostics = {}
    for _, diagnostic in ipairs(vim.diagnostic.get(bufnr)) do
      local start = { diagnostic.lnum + 1, diagnostic.col }
      local finish = {
        (diagnostic.end_lnum or diagnostic.lnum) + 1,
        diagnostic.end_col or diagnostic.col,
      }
      local diagnostic_range = {
        start = convert.to_lsp_position(bufnr, start, "utf-16"),
        ["end"] = convert.to_lsp_position(bufnr, finish, "utf-16"),
      }
      if ranges_intersect(diagnostic_range, resolved_range) then
        local converted = vim.deepcopy(diagnostic.user_data and diagnostic.user_data.lsp or {
          message = diagnostic.message, severity = diagnostic.severity,
          code = diagnostic.code, source = diagnostic.source,
        })
        converted.range = {
          start = convert.to_lsp_position(bufnr, start, client.offset_encoding),
          ["end"] = convert.to_lsp_position(bufnr, finish, client.offset_encoding),
        }
        diagnostics[#diagnostics + 1] = converted
      end
    end
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
        if not vim.tbl_contains(merged, function(existing) return same_action(existing, action) end, { predicate = true }) then
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
      local request_context = response.context or response.ctx
      for _, action in ipairs(response.result ~= vim.NIL and response.result or {}) do
        local item = vim.deepcopy(action)
        item._mcdev_client_id = client_id
        if request_context and request_context.params then
          item._mcdev_request_params = vim.deepcopy(request_context.params)
        end
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
