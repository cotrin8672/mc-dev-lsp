local config = require("mcdev.config")
local buffer = require("mcdev.buffer")
local convert = require("mcdev.convert")
local ui = require("mcdev.ui")

local M = {}

M.VERSION = 1
local provider_id = "mcdev.mixin"
local provider_command = "ktlsp.providerCommand"
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
  if type(path) ~= "string" or path == "" then
    return nil
  end
  if path:sub(1, 5) == "file:" then
    local ok, converted = pcall(vim.uri_to_fname, path)
    if not ok then return nil end
    path = converted
  end
  local normalized = vim.fs and vim.fs.normalize and vim.fs.normalize(path) or path:gsub("\\", "/")
  if package.config:sub(1, 1) == "\\" and normalized:sub(1, 1) == "/" and normalized:sub(1, 2) ~= "//" then
    -- Neovim 0.11 adds a drive to buffer names on Windows.
    local cwd = ((vim.uv or vim.loop).cwd() or ""):gsub("\\", "/")
    local volume = cwd:match("^%a:") or cwd:match("^//[^/]+/[^/]+")
    if volume then normalized = volume .. normalized end
  end
  if normalized:match("^/+$") then return "/" end
  if normalized:match("^%a:/$") then return normalized end
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
  local prefix = normalized_root:sub(-1) == "/" and normalized_root or normalized_root .. "/"
  return normalized_path == normalized_root or normalized_path:sub(1, #prefix) == prefix
end

local function inferred_workspace_root(bufnr)
  local path = buffer_path(bufnr)
  local start = path or vim.fn.getcwd()
  if vim.fs and vim.fs.root then
    return normalize_path(vim.fs.root(start, root_markers))
  end
  return normalize_path(vim.fn.getcwd())
end

local function advertised(client, command)
  local capability = client.server_capabilities and client.server_capabilities.executeCommandProvider or {}
  local commands = type(capability) == "table" and capability.commands or nil
  for _, registered in ipairs(type(commands) == "table" and commands or {}) do
    if registered == command then return true end
  end
  return false
end

local function provider_config(client)
  local cfg = client and client.config or {}
  local init = type(cfg.init_options) == "table" and cfg.init_options or {}
  local initial = type(init.analysisProviders) == "table" and init.analysisProviders[provider_id] or nil
  local settings = client and client.settings or cfg.settings
  settings = type(settings) == "table" and settings or {}
  local current = type(settings.analysisProviders) == "table" and settings.analysisProviders[provider_id] or nil
  if type(current) == "table" then
    if current.enabled == false or (type(initial) == "table" and initial.enabled == false
      and current.enabled ~= true) then return nil end
    if current.command ~= nil or type(initial) ~= "table" then return current end
    return {
      command = initial.command,
      executeCommands = current.executeCommands ~= nil and current.executeCommands or initial.executeCommands,
    }
  end
  if type(initial) == "table" and initial.enabled ~= false then return initial end
  return nil
end

local function provider_registered(client)
  local provider = provider_config(client)
  if not provider or type(provider.command) ~= "table" or #provider.command == 0
    or type(provider.executeCommands) ~= "table" then return false end
  for _, part in ipairs(provider.command) do
    if type(part) ~= "string" or part == "" or part:find("\0", 1, true) then return false end
  end
  return true
end

local function provider_disabled(client)
  local cfg = client and client.config or {}
  local init = type(cfg.init_options) == "table" and cfg.init_options or {}
  local initial = type(init.analysisProviders) == "table" and init.analysisProviders[provider_id] or nil
  local settings = client and client.settings or cfg.settings
  local current = type(settings) == "table" and type(settings.analysisProviders) == "table"
    and settings.analysisProviders[provider_id] or nil
  if type(current) == "table" and current.enabled ~= nil then return current.enabled == false end
  return type(initial) == "table" and initial.enabled == false
end

local function uses_provider_command(client)
  return client and advertised(client, provider_command) and provider_registered(client)
end

local function advertises_legacy_command(client)
  for _, command in pairs(M.commands) do
    if advertised(client, command) then return true end
  end
  return false
end

local function is_host(client)
  local cfg = client.config or {}
  local init = type(cfg.init_options) == "table" and cfg.init_options or {}
  local settings = type(client.settings or cfg.settings) == "table" and (client.settings or cfg.settings) or {}
  if type(init.analysisProviders) == "table" and init.analysisProviders[provider_id] ~= nil then return true end
  if type(settings.analysisProviders) == "table" and settings.analysisProviders[provider_id] ~= nil then return true end
  if advertised(client, provider_command) then return true end
  return advertises_legacy_command(client)
end

function M.active_ktlsp_client(bufnr, command)
  bufnr = bufnr or 0
  local path = buffer_path(bufnr)
  local inferred_root = inferred_workspace_root(bufnr)
  local selected, selected_length = nil, -1
  local function select(clients, attached)
    for _, client in ipairs(clients) do
      local root = client_root(client)
      local matches = root and path_under(root, path)
      if not advertised(client, provider_command) then
        matches = matches or (attached and not root) or (root and inferred_root
          and comparable_path(root) == comparable_path(inferred_root))
      end
      if is_host(client) and matches then
        local length = root and #root or 0
        if length > selected_length then selected, selected_length = client, length end
      end
    end
  end
  select(vim.lsp.get_clients({ bufnr = bufnr }), true)
  select(vim.lsp.get_clients(), false)
  if not selected or provider_disabled(selected)
    or (advertised(selected, provider_command) and not provider_registered(selected))
    or (not advertised(selected, provider_command) and not advertises_legacy_command(selected))
    or (command and not M.command_supported(selected, command)) then return nil end
  return selected
end

function M.command_supported(client, command)
  if not client or provider_disabled(client) then return false end
  local provider = provider_config(client)
  if provider and provider.executeCommands ~= nil then
    if type(provider.executeCommands) ~= "table" then return false end
    local allowed = false
    for _, id in ipairs(provider.executeCommands) do
      if id == command then allowed = true; break end
    end
    if not allowed then return false end
  end
  if advertised(client, provider_command) then
    if not provider_registered(client) then return false end
    for _, allowed in ipairs(provider.executeCommands) do
      if allowed == command then return true end
    end
    return false
  end
  local capabilities = client and client.server_capabilities or {}
  local provider = capabilities.executeCommandProvider or {}
  local commands = type(provider) == "table" and provider.commands or nil
  for _, registered in ipairs(type(commands) == "table" and commands or {}) do
    if registered == command then return true end
  end
  return false
end

local function resolved_bufnr(bufnr)
  return (not bufnr or bufnr == 0) and vim.api.nvim_get_current_buf() or bufnr
end

local function lsp_text(bufnr)
  return vim.lsp._buf_get_full_text(resolved_bufnr(bufnr))
end

local function lsp_version(bufnr, client)
  local versions = vim.lsp.util and vim.lsp.util.buf_versions
  bufnr = resolved_bufnr(bufnr)
  local version = type(versions) == "table" and rawget(versions, bufnr) or nil
  if version == nil and type(versions) == "table" and client and client.id
    and vim.lsp.get_client_by_id(client.id) == client and client.initialized
    and client.attached_buffers and client.attached_buffers[bufnr]
    and client:supports_method("textDocument/didOpen") then
    -- Neovim's didOpen sends its default buffer version even before any edit.
    version = versions[bufnr]
  end
  return type(version) == "number" and version or nil
end

local function workspace_root(bufnr)
  local client = M.active_ktlsp_client(bufnr)
  local root = client_root(client) or inferred_workspace_root(bufnr) or vim.fn.getcwd()
  return vim.uri_from_fname(root)
end

function M.context(bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  position = position or vim.api.nvim_win_get_cursor(0)
  local client = M.active_ktlsp_client(bufnr)
  local provider = uses_provider_command(client)
  return {
    protocolVersion = M.VERSION,
    workspaceRoot = workspace_root(bufnr),
    documentUri = document_uri(bufnr),
    languageId = buffer.effective_language_id(bufnr),
    position = convert.to_lsp_position(bufnr, position),
    documentVersion = provider and lsp_version(bufnr, client) or (not provider and vim.api.nvim_buf_get_changedtick(bufnr) or nil),
    bufferText = provider and lsp_text(bufnr) or table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
    client = {
      name = "mcdev.nvim",
      version = "0.1.0",
    },
  }
end

local function noop_cancel()
end

function M.request(command, payload, callback, bufnr)
  bufnr = bufnr or 0
  local client = M.active_ktlsp_client(bufnr, command) or M.active_ktlsp_client(bufnr)
  if not client then
    local message = "mcdev: no active ktlsp client for this buffer"
    if callback then
      callback(nil, message)
      return noop_cancel
    end
    vim.notify(message, vim.log.levels.WARN)
    return noop_cancel
  end

  if not M.command_supported(client, command) then
    local message = "mcdev: ktlsp has not advertised command " .. tostring(command)
    if callback then callback(nil, message)
    else vim.notify(message, vim.log.levels.WARN) end
    return noop_cancel
  end

  local params = { command = command, arguments = { payload } }
  if uses_provider_command(client) then
    local actual_bufnr = resolved_bufnr(bufnr)
    if type(client.attached_buffers) == "table" and not client.attached_buffers[actual_bufnr] then
      if not client.id or not vim.lsp.buf_attach_client(actual_bufnr, client.id) then
        local message = "mcdev: could not attach ktlsp to the current document"
        if callback then callback(nil, message) else vim.notify(message, vim.log.levels.WARN) end
        return noop_cancel
      end
    end
    local current_text, current_version = lsp_text(actual_bufnr), lsp_version(actual_bufnr, client)
    local uri, root = document_uri(actual_bufnr), client_root(client)
    local inner = type(payload) == "table" and payload.context or nil
    if type(inner) == "table" then
      local text = type(inner.bufferText) == "string" and inner.bufferText or inner.bufferTextFallback
      local wrong_text = type(text) == "string" and text ~= current_text
      local wrong_version = type(inner.documentVersion) == "number" and inner.documentVersion ~= current_version
      local wrong_uri = type(inner.documentUri) == "string" and inner.documentUri ~= uri
      local wrong_root = type(inner.workspaceRoot) == "string"
        and comparable_path(inner.workspaceRoot) ~= comparable_path(root)
      if wrong_text or wrong_version or wrong_uri or wrong_root then
        local message = "mcdev: request context is stale or belongs to another document"
        if callback then callback(nil, message) else vim.notify(message, vim.log.levels.WARN) end
        return noop_cancel
      end
    end
    params = {
      command = provider_command,
      arguments = {{
        provider = provider_id,
        command = command,
        context = {
          workspaceRoot = root,
          documentUri = uri,
          bufferText = current_text,
          documentVersion = current_version,
        },
        arguments = { payload },
        refreshProject = (command == M.commands.reindex or command == M.commands.reload_project_context) and true or nil,
      }},
    }
  end
  local origin = {
    client = client,
    client_id = client.id,
    command = command,
    route = params.command,
    provider = params.command == provider_command and provider_id or nil,
  }
  local status, request_id = client:request("workspace/executeCommand", params, function(err, result, context)
    if callback then
      local matches = context == nil or (type(context) == "table"
        and (context.client_id == nil or context.client_id == origin.client_id))
      callback(result, err, matches and origin or nil)
    end
  end, bufnr)

  if not status or not request_id then
    local message = "mcdev: failed to start ktlsp request"
    if callback then
      callback(nil, message)
    else
      vim.notify(message, vim.log.levels.WARN)
    end
    return noop_cancel
  end

  local cancelled = false
  return function()
    if cancelled then
      return
    end
    cancelled = true
    client:cancel_request(request_id)
  end
end

function M.build_completion_payload(bufnr, position)
  local opts = config.options.insert
  local ctx = M.context(bufnr, position)
  ctx.bufferTextFallback = ctx.bufferText
  ctx.bufferText = nil
  return {
    context = ctx,
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
  return M.request(M.commands.completion, payload, callback, bufnr)
end

function M.build_code_action_payload(bufnr, range, diagnostic_codes)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local context = M.context(bufnr)
  if range then
    context.position = range.start
  end
  local resolved_range = range or {
    start = {
      line = context.position.line,
      character = context.position.character,
    },
    ["end"] = {
      line = context.position.line,
      character = context.position.character,
    },
  }
  return {
    context = context,
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
    local payload, unwrap_err = convert.unwrap_envelope(result, err)
    if unwrap_err then
      ui.error(unwrap_err)
      return
    end
    local lines = vim.tbl_map(function(line)
      return line:gsub("^Extension jar location:", "Companion executable:")
        :gsub("^Extension build commit:", "Companion build commit:")
        :gsub("^Extension build time:", "Companion build time:")
    end, payload.lines or {})
    ui.show_report("mcdev info", lines)
  end)
end

function M.reindex()
  M.request(M.commands.reindex, { context = M.context() }, function(result, err)
    local payload, unwrap_err = convert.unwrap_envelope(result, err)
    if unwrap_err then
      ui.error(unwrap_err)
      return
    end
    vim.notify(string.format(
      "mcdev: %s (index: %s, classpath: %s)",
      payload.status or "reindex complete",
      payload.indexState or "unknown",
      tostring(payload.classpathEntries or "unknown")
    ))
  end)
end

function M.reload_project_context()
  M.request(M.commands.reload_project_context, { context = M.context() }, function(result, err)
    local payload, unwrap_err = convert.unwrap_envelope(result, err)
    if unwrap_err then
      ui.error(unwrap_err)
      return
    end
    local lines = { payload.status or "project context reloaded" }
    vim.list_extend(lines, payload.lines or {})
    ui.show_report("mcdev project context", lines)
  end)
end

function M.dump_context()
  M.request(M.commands.dump_context, { context = M.context() }, function(result, err)
    local payload, unwrap_err = convert.unwrap_envelope(result, err)
    if unwrap_err then
      ui.error(unwrap_err)
      return
    end
    local lines = payload.lines or {}
    if #lines > 0 then
      ui.show_report("mcdev project context", lines)
    else
      ui.show_report("mcdev project context", vim.split(vim.inspect(payload), "\n", { plain = true }))
    end
  end)
end

return M
