local config = require("mcdev.config")
local buffer = require("mcdev.buffer")

local M = {}

M.VERSION = 1
M.commands = {
  completion = "mcdev.completion",
  definition = "mcdev.definition",
  references = "mcdev.references",
  code_action = "mcdev.codeAction",
  reindex = "mcdev.reindex",
  context = "mcdev.context",
  info = "mcdev.info",
}

local function document_uri(bufnr)
  return vim.uri_from_bufnr(bufnr)
end

local function workspace_root(bufnr)
  local clients = vim.lsp.get_clients({ bufnr = bufnr or 0 })
  for _, client in ipairs(clients) do
    if client.name == "jdtls" and client.config and client.config.root_dir then
      return vim.uri_from_fname(client.config.root_dir)
    end
  end
  return vim.uri_from_fname(vim.fn.getcwd())
end

function M.active_jdtls_client(bufnr)
  local clients = vim.lsp.get_clients({ bufnr = bufnr or 0 })
  for _, client in ipairs(clients) do
    if client.name == "jdtls" then
      return client
    end
  end
  return nil
end

function M.context(bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  return {
    protocolVersion = M.VERSION,
    workspaceRoot = workspace_root(bufnr),
    documentUri = document_uri(bufnr),
    languageId = buffer.effective_language_id(bufnr),
    position = {
      line = position[1] - 1,
      character = position[2],
    },
    bufferText = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
    client = {
      name = "mcdev.nvim",
      version = "0.1.0",
    },
  }
end

function M.request(command, payload, callback, bufnr)
  bufnr = bufnr or 0
  local client = M.active_jdtls_client(bufnr)
  if not client then
    local message = "mcdev: no active JDT LS client for this buffer"
    if callback then
      callback(nil, message)
      return
    end
    vim.notify(message, vim.log.levels.WARN)
    return
  end

  client.request("workspace/executeCommand", {
    command = command,
    arguments = { payload },
  }, function(err, result)
    if callback then
      callback(result, err)
    end
  end, bufnr)
end

function M.build_completion_payload(bufnr, position)
  local opts = config.options.mappings
  return {
    context = M.context(bufnr, position),
    trigger = {
      kind = "manual",
      character = nil,
    },
    options = {
      preferredAtTarget = opts.preferred_at_target,
      mixinClassInsert = opts.mixin_class_insert,
      injectMethodDescriptor = opts.inject_method_descriptor,
    },
  }
end

function M.completion(callback, bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local payload = M.build_completion_payload(bufnr, position)
  M.request(M.commands.completion, payload, callback, bufnr)
end

function M.build_code_action_payload(bufnr, range, diagnostic_codes)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local position = range and { range.start.line + 1, range.start.character }
    or vim.api.nvim_win_get_cursor(0)
  local resolved_range = range or {
    start = {
      line = position[1] - 1,
      character = position[2],
    },
    ["end"] = {
      line = position[1] - 1,
      character = position[2],
    },
  }
  return {
    context = M.context(bufnr, position),
    range = resolved_range,
    diagnosticCodes = diagnostic_codes or {},
  }
end

function M.definition(bufnr, position, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  M.request(M.commands.definition, { context = M.context(bufnr, position) }, callback, bufnr)
end

function M.references(bufnr, position, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  M.request(M.commands.references, { context = M.context(bufnr, position) }, callback, bufnr)
end

function M.diagnostics(bufnr, position, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  M.request(M.commands.context, { context = M.context(bufnr, position) }, callback, bufnr)
end

function M.code_action(bufnr, range, diagnostic_codes, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local payload = M.build_code_action_payload(bufnr, range, diagnostic_codes)
  M.request(M.commands.code_action, payload, callback, bufnr)
end

function M.info()
  M.request(M.commands.info, { context = M.context() }, function(result, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    local envelope = result or {}
    local lines = envelope.result and envelope.result.lines or {}
    vim.notify(table.concat(lines, "\n"))
  end)
end

function M.reindex()
  M.request(M.commands.reindex, { context = M.context() }, function(_, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
    else
      vim.notify("mcdev: reindex requested")
    end
  end)
end

return M
