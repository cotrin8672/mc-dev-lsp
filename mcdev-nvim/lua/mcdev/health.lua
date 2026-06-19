local config = require("mcdev.config")
local diagnostics = require("mcdev.diagnostics")
local completion = require("mcdev.completion")
local protocol = require("mcdev.protocol")

local M = {}

local function bool_text(value)
  return value and "yes" or "no"
end

local function client_id(client)
  return client and tostring(client.id or "?") or "none"
end

local function client_root(client)
  return client and client.config and client.config.root_dir or "none"
end

local function module_path(name)
  local ok, module = pcall(require, name)
  if ok and type(module) == "table" then
    for _, value in pairs(module) do
      if type(value) == "function" then
        local info = debug.getinfo(value, "S")
        if info and info.source and info.source ~= "" then
          return info.source:gsub("^@", "")
        end
      end
    end
  end
  local path = package.searchpath(name, package.path)
  return path or "not found"
end

local function current_commit()
  local ok, lines = pcall(vim.fn.systemlist, { "git", "rev-parse", "HEAD" })
  if ok and vim.v.shell_error == 0 and lines and lines[1] and lines[1] ~= "" then
    return lines[1]
  end
  return "unknown"
end

local function extension_jar()
  local ok, jar = pcall(function()
    return require("mcdev.jdtls").resolve_extension_jar()
  end)
  if ok then
    return jar
  end
  return nil
end

local function first_items(items, limit)
  local result = {}
  for index = 1, math.min(#items, limit) do
    result[#result + 1] = items[index]
  end
  return result
end

local function envelope_error(envelope, err)
  if err then
    return tostring(err)
  end
  if envelope and envelope.error then
    return vim.inspect(envelope.error)
  end
  return nil
end

local function ping(command, payload, bufnr, done)
  local started = vim.loop.hrtime()
  protocol.request(command, payload, function(envelope, err)
    done({
      ok = envelope_error(envelope, err) == nil,
      envelope = envelope,
      err = envelope_error(envelope, err),
      elapsed_ms = math.floor((vim.loop.hrtime() - started) / 1000000),
    })
  end, bufnr)
end

local function base_lines(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local client = protocol.active_jdtls_client(bufnr)
  local jar = extension_jar()
  local blink_loaded = package.loaded["blink.cmp"] ~= nil or pcall(require, "blink.cmp")
  local cmp_loaded = package.loaded["cmp"] ~= nil or pcall(require, "cmp")
  return {
    "mcdev-nvim loaded: yes",
    "mcdev-nvim module path: " .. module_path("mcdev"),
    "mcdev.blink module path: " .. module_path("mcdev.blink"),
    "mcdev.completion module path: " .. module_path("mcdev.completion"),
    "jdtls client found: " .. bool_text(client ~= nil),
    "selected jdtls client id: " .. client_id(client),
    "selected jdtls root_dir: " .. tostring(client_root(client)),
    "workspace root: " .. protocol.context(bufnr).workspaceRoot,
    "current buffer uri: " .. vim.uri_from_bufnr(bufnr),
    "current filetype: " .. vim.bo[bufnr].filetype,
    "current language id: " .. require("mcdev.buffer").effective_language_id(bufnr),
    "resolved extension jar: " .. tostring(jar),
    "extension jar readable: " .. bool_text(jar ~= nil and vim.fn.filereadable(jar) == 1),
    "blink.cmp detected: " .. bool_text(blink_loaded),
    "blink provider module: mcdev.blink",
    "nvim-cmp detected: " .. bool_text(cmp_loaded),
    "nvim-cmp source module: mcdev.cmp",
    "omnifunc configured: " .. tostring(vim.bo[bufnr].omnifunc),
    "diagnostics config:",
    "  enabled: " .. tostring(config.options.diagnostics.enabled),
    "  events: " .. table.concat(config.options.diagnostics.events or {}, ", "),
    "  debounce_ms: " .. tostring(config.options.diagnostics.debounce_ms),
    "  insert_mode: " .. tostring(config.options.diagnostics.insert_mode),
    "diagnostics enabled: " .. tostring(config.options.diagnostics.enabled),
    "diagnostics running: " .. tostring(diagnostics.running),
    "diagnostics events: " .. table.concat(config.options.diagnostics.events or {}, ", "),
    "diagnostics request count: " .. tostring(diagnostics.request_count),
    "last diagnostics trigger event: " .. tostring(diagnostics.last_trigger_event or "none"),
    "completion adapter last source: " .. tostring(completion.last_source or "none"),
    "completion adapter request count: " .. tostring(completion.request_count),
    "completion adapter last callback item count: " .. tostring(completion.last_callback_item_count),
    "last completion request: " .. vim.inspect(completion.last_request),
    "last completion response count: " .. tostring(completion.last_response_count),
    "last completion error: " .. tostring(completion.last_error),
    "last completion debug: " .. vim.inspect(completion.last_debug),
    "last diagnostics request: " .. vim.inspect(diagnostics.last_request),
    "last diagnostics error: " .. tostring(diagnostics.last_error),
  }
end

function M.lines(bufnr)
  return base_lines(bufnr)
end

function M.health(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local position = vim.api.nvim_win_get_cursor(0)
  local payloads = {
    info = { command = protocol.commands.info, payload = { context = protocol.context(bufnr, position) } },
    completion = { command = protocol.commands.completion, payload = protocol.build_completion_payload(bufnr, position) },
    diagnostics = { command = protocol.commands.diagnostics, payload = { context = protocol.context(bufnr, position) } },
    hover = { command = protocol.commands.hover, payload = { context = protocol.context(bufnr, position) } },
  }
  local results = {}
  local pending = 0
  for _ in pairs(payloads) do
    pending = pending + 1
  end

  local function finish_one(name, result)
    results[name] = result
    pending = pending - 1
    if pending > 0 then
      return
    end

    local info = results.info.envelope and results.info.envelope.result or {}
    local completion_result = results.completion.envelope and results.completion.envelope.result or {}
    local completion_items = completion_result.items or {}
    local debug = completion_result.debug or {}
    local labels = {}
    for index = 1, math.min(#completion_items, 10) do
      labels[#labels + 1] = completion_items[index].label
    end
    local lines = base_lines(bufnr)
    vim.list_extend(lines, {
      "repo current commit: " .. current_commit(),
      "jar build commit: " .. tostring(info.buildCommit or "unknown"),
      "extension build commit: " .. tostring(info.buildCommit or "unknown"),
      "extension build time: " .. tostring(info.buildTime or "unknown"),
      "extension jar location: " .. tostring(info.jarLocation or "unknown"),
      "registered commands: " .. vim.inspect(info.registeredCommands or {}),
      "mcdev.info ping: " .. (results.info.ok and "OK" or "FAILED"),
      "mcdev.info error: " .. tostring(results.info.err),
      "mcdev.completion ping: " .. (results.completion.ok and "OK" or "FAILED"),
      "mcdev.completion itemCount: " .. tostring(#completion_items),
      "mcdev.completion zeroItemReason: " .. tostring(debug.zeroItemReason),
      "mcdev.completion error: " .. tostring(results.completion.err),
      "mcdev.diagnostics ping: " .. (results.diagnostics.ok and "OK" or "FAILED"),
      "mcdev.diagnostics error: " .. tostring(results.diagnostics.err),
      "mcdev.hover ping: " .. (results.hover.ok and "OK" or "FAILED"),
      "mcdev.hover error: " .. tostring(results.hover.err),
      "completion probe:",
      "  command sent: " .. protocol.commands.completion,
      "  item count: " .. tostring(#completion_items),
      "  first 10 labels: " .. vim.inspect(labels),
      "  zeroItemReason: " .. tostring(debug.zeroItemReason),
      "  parseSource: " .. tostring(debug.parseSource),
      "  parseConfidence: " .. tostring(debug.parseConfidence),
      "  usedCompilationUnit: " .. tostring(debug.usedCompilationUnit),
      "  usedJavaProject: " .. tostring(debug.usedJavaProject),
      "  bindingResolvedCount: " .. tostring(debug.bindingResolvedCount),
      "  bindingFailedCount: " .. tostring(debug.bindingFailedCount),
      "  semanticTargetCount: " .. tostring(debug.semanticTargetCount),
      "  semanticMemberCount: " .. tostring(debug.semanticMemberCount),
      "  semanticContextFound: " .. tostring(debug.semanticContextFound),
      "  fallbackAnnotationContextUsed: " .. tostring(debug.fallbackAnnotationContextUsed),
      "  fallbackReason: " .. tostring(debug.fallbackReason),
      "  completionContextKind: " .. tostring(debug.completionContextKind),
      "  warnings: " .. vim.inspect(debug.warnings or {}),
    })
    if info.buildCommit and current_commit() ~= "unknown" and info.buildCommit ~= current_commit() then
      lines[#lines + 1] = "WARNING: JDTLS is using an old mcdev extension jar. Rebuild/reinstall the jar and restart JDTLS."
    end
    vim.notify(table.concat(lines, "\n"))
  end

  for name, request in pairs(payloads) do
    ping(request.command, request.payload, bufnr, function(result)
      finish_one(name, result)
    end)
  end
end

function M.debug_completion(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local position = vim.api.nvim_win_get_cursor(0)
  local started = vim.loop.hrtime()
  local payload = protocol.build_completion_payload(bufnr, position)
  protocol.completion(function(envelope, err)
    local elapsed_ms = math.floor((vim.loop.hrtime() - started) / 1000000)
    local result = envelope and envelope.result or {}
    local items = result.items or {}
    local debug = result.debug or {}
    local labels = {}
    for index = 1, math.min(#items, 20) do
      labels[#labels + 1] = items[index].label
    end
    vim.notify(table.concat({
      "request params: " .. vim.inspect(payload),
      "selected jdtls client: " .. client_id(protocol.active_jdtls_client(bufnr)),
      "raw response: " .. vim.inspect(envelope),
      "item count: " .. tostring(#items),
      "zero item reason: " .. tostring(debug.zeroItemReason),
      "parse source: " .. tostring(debug.parseSource),
      "parse confidence: " .. tostring(debug.parseConfidence),
      "used compilation unit/project: " .. tostring(debug.usedCompilationUnit) .. "/" .. tostring(debug.usedJavaProject),
      "binding resolved/failed: " .. tostring(debug.bindingResolvedCount) .. "/" .. tostring(debug.bindingFailedCount),
      "fallback reason: " .. tostring(debug.fallbackReason),
      "semantic context found: " .. tostring(debug.semanticContextFound),
      "fallback annotation context used: " .. tostring(debug.fallbackAnnotationContextUsed),
      "semantic target/member count: " .. tostring(debug.semanticTargetCount) .. "/" .. tostring(debug.semanticMemberCount),
      "completion context kind: " .. tostring(debug.completionContextKind),
      "owner: " .. tostring(debug.owner),
      "method: " .. tostring(debug.methodName) .. tostring(debug.methodDescriptor or ""),
      "warnings: " .. vim.inspect(debug.warnings or {}),
      "first 20 labels: " .. vim.inspect(labels),
      "error: " .. tostring(err),
      "elapsed ms: " .. tostring(elapsed_ms),
    }, "\n"))
  end, bufnr, position)
end

function M.debug_diagnostics(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local position = vim.api.nvim_win_get_cursor(0)
  local started = vim.loop.hrtime()
  protocol.diagnostics(bufnr, position, function(envelope, err)
    local elapsed_ms = math.floor((vim.loop.hrtime() - started) / 1000000)
    local result = envelope and envelope.result or {}
    local items = result.diagnostics or {}
    vim.notify(table.concat({
      "request params: " .. vim.inspect({ context = protocol.context(bufnr, position) }),
      "diagnostic count: " .. tostring(#items),
      "first diagnostics: " .. vim.inspect(first_items(items, 10)),
      "error: " .. tostring(err),
      "elapsed ms: " .. tostring(elapsed_ms),
      "stale result dropped: " .. tostring(diagnostics.last_dropped_stale),
    }, "\n"))
  end)
end

return M
