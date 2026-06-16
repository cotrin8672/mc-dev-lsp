local config = require("mcdev.config")
local diagnostics = require("mcdev.diagnostics")
local protocol = require("mcdev.protocol")

local M = {}

local function bool_text(value)
  return value and "yes" or "no"
end

local function client_id(client)
  return client and tostring(client.id or "?") or "none"
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

local function command_lines()
  local commands = protocol.commands
  return {
    "  - " .. commands.info,
    "  - " .. commands.completion,
    "  - " .. commands.hover,
    "  - " .. commands.diagnostics,
  }
end

local function first_items(items, limit)
  local result = {}
  for index = 1, math.min(#items, limit) do
    result[#result + 1] = items[index]
  end
  return result
end

function M.lines(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local client = protocol.active_jdtls_client(bufnr)
  local jar = extension_jar()
  local blink_loaded = package.loaded["blink.cmp"] ~= nil or pcall(require, "blink.cmp")
  local cmp_loaded = package.loaded["cmp"] ~= nil or pcall(require, "cmp")
  local lines = {
    "mcdev-nvim loaded: yes",
    "jdtls client found: " .. bool_text(client ~= nil),
    "jdtls client id: " .. client_id(client),
    "workspace root: " .. protocol.context(bufnr).workspaceRoot,
    "current buffer uri: " .. vim.uri_from_bufnr(bufnr),
    "current filetype: " .. vim.bo[bufnr].filetype,
    "current language id: " .. require("mcdev.buffer").effective_language_id(bufnr),
    "mcdev extension commands responding:",
  }
  vim.list_extend(lines, command_lines())
  vim.list_extend(lines, {
    "extension bundle path: " .. tostring(jar),
    "extension bundle readable: " .. bool_text(jar ~= nil and vim.fn.filereadable(jar) == 1),
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
    "last completion request: " .. vim.inspect(require("mcdev.completion").last_request),
    "last completion response count: " .. tostring(require("mcdev.completion").last_response_count),
    "last completion error: " .. tostring(require("mcdev.completion").last_error),
    "last diagnostics request: " .. vim.inspect(diagnostics.last_request),
    "last diagnostics error: " .. tostring(diagnostics.last_error),
  })
  return lines
end

function M.health(bufnr)
  vim.notify(table.concat(M.lines(bufnr), "\n"))
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
    local labels = {}
    for index = 1, math.min(#items, 20) do
      labels[#labels + 1] = items[index].label
    end
    vim.notify(table.concat({
      "request params: " .. vim.inspect(payload),
      "selected jdtls client: " .. client_id(protocol.active_jdtls_client(bufnr)),
      "raw response: " .. vim.inspect(envelope),
      "item count: " .. tostring(#items),
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
