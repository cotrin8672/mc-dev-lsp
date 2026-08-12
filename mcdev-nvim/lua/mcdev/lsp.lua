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
  local standard_actions = nil
  local standard_error = nil
  local mcdev_actions = nil
  local mcdev_error = nil

  local function finish()
    if standard_actions == nil or mcdev_actions == nil then
      return
    end
    local merged = {}
    local seen = {}
    for _, actions in ipairs({ standard_actions, mcdev_actions }) do
      for _, action in ipairs(actions) do
        local key = tostring(action.title or action.command or "") .. "\0" .. tostring(action.kind or "")
        if not seen[key] then
          seen[key] = true
          merged[#merged + 1] = action
        end
      end
    end
    local err = #merged == 0 and (standard_error or mcdev_error) or nil
    if cb then cb(merged, err) end
  end

  local function collect_standard(results)
    standard_actions = {}
    for _, response in pairs(results or {}) do
      local response_error = response.err or response.error
      if response_error and not standard_error then
        standard_error = type(response_error) == "table" and (response_error.message or vim.inspect(response_error))
          or tostring(response_error)
      end
      for _, action in ipairs(response.result or {}) do
        standard_actions[#standard_actions + 1] = action
      end
    end
    finish()
  end

  if vim.lsp.buf_request_all then
    vim.lsp.buf_request_all(bufnr, "textDocument/codeAction", params, collect_standard)
  else
    vim.lsp.buf_request(bufnr, "textDocument/codeAction", params, function(err, result)
      collect_standard({ { error = err, result = result } })
    end)
  end

  code_action.code_actions(bufnr, range, diagnostic_codes, function(actions, err)
    mcdev_actions = actions or {}
    mcdev_error = err
    finish()
  end)
end

return M
