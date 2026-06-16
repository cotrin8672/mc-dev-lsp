local config = require("mcdev.config")
local buffer = require("mcdev.buffer")

local M = {}

M.VERSION = 1
M.commands = {
  completion = "mcdev.completion",
  definition = "mcdev.definition",
  references = "mcdev.references",
  hover = "mcdev.hover",
  code_action = "mcdev.codeAction",
  diagnostics = "mcdev.diagnostics",
  reindex = "mcdev.reindex",
  reload_project_context = "mcdev.reloadProjectContext",
  dump_context = "mcdev.dumpContext",
  context = "mcdev.context",
  info = "mcdev.info",
}

local function document_uri(bufnr)
  return vim.uri_from_bufnr(bufnr)
end

local root_markers = { "gradlew", "build.gradle", "build.gradle.kts", "pom.xml", ".git" }

local function normalize_path(path)
  if not path or path == "" then
    return nil
  end
  local normalized = vim.fs and vim.fs.normalize and vim.fs.normalize(path) or path:gsub("\\", "/")
  return normalized:gsub("/+$", "")
end

local function comparable_path(path)
  local normalized = normalize_path(path)
  if not normalized then
    return nil
  end
  if package.config:sub(1, 1) == "\\" then
    return normalized:lower()
  end
  return normalized
end

local function client_root(client)
  return client and client.config and normalize_path(client.config.root_dir) or nil
end

local function buffer_path(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  return normalize_path(vim.api.nvim_buf_get_name(bufnr))
end

local function path_under(root, path)
  local normalized_root = comparable_path(root)
  local normalized_path = comparable_path(path)
  if not normalized_root or not normalized_path then
    return false
  end
  return normalized_path == normalized_root
    or normalized_path:sub(1, #normalized_root + 1) == normalized_root .. "/"
end

local function inferred_workspace_root(bufnr)
  local path = buffer_path(bufnr)
  local start = path or vim.fn.getcwd()
  if vim.fs and vim.fs.root then
    return normalize_path(vim.fs.root(start, root_markers))
  end
  return normalize_path(vim.fn.getcwd())
end

function M.active_jdtls_client(bufnr)
  bufnr = bufnr or 0
  local clients = vim.lsp.get_clients({ bufnr = bufnr })
  for _, client in ipairs(clients) do
    if client.name == "jdtls" then
      return client
    end
  end

  local path = buffer_path(bufnr)
  local inferred_root = inferred_workspace_root(bufnr)
  for _, client in ipairs(vim.lsp.get_clients({ name = "jdtls" })) do
    local root = client_root(client)
    if path_under(root, path) or comparable_path(root) == comparable_path(inferred_root) then
      return client
    end
  end

  return nil
end

local function workspace_root(bufnr)
  local client = M.active_jdtls_client(bufnr)
  local root = client_root(client) or inferred_workspace_root(bufnr) or vim.fn.getcwd()
  return vim.uri_from_fname(root)
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
  local opts = config.options.insert
  return {
    context = M.context(bufnr, position),
    trigger = {
      kind = "manual",
      character = nil,
    },
    options = {
      preferredAtTarget = opts.at_target,
      mixinClassInsert = opts.mixin_class_import and "import" or "fqn",
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

function M.hover(bufnr, position, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  M.request(M.commands.hover, { context = M.context(bufnr, position) }, callback, bufnr)
end

function M.diagnostics(bufnr, position, callback)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  M.request(M.commands.diagnostics, { context = M.context(bufnr, position) }, callback, bufnr)
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

function M.reload_project_context()
  M.request(M.commands.reload_project_context, { context = M.context() }, function(result, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    local envelope = result or {}
    local payload = envelope.result or {}
    vim.notify(payload.status or "mcdev: project context reloaded")
  end)
end

function M.dump_context()
  M.request(M.commands.dump_context, { context = M.context() }, function(result, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    local envelope = result or {}
    local payload = envelope.result or {}
    local lines = payload.lines or {}
    if #lines > 0 then
      vim.notify(table.concat(lines, "\n"))
    else
      vim.notify(vim.inspect(payload))
    end
  end)
end

return M
