local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")
local ktlsp = require("mcdev.ktlsp")

local commands = vim.tbl_values(protocol.commands)
table.sort(commands)
local saved_clients, saved_attach = vim.lsp.get_clients, vim.lsp.buf_attach_client
local root = vim.fs.normalize(vim.fn.tempname())
local parent_root = vim.fs.dirname(root)
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, root .. "/src/Example.java")
vim.bo[bufnr].filetype = "java"
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class 猫 {", "  // 🙂", "}" })
local saved_version = vim.lsp.util.buf_versions[bufnr]
vim.lsp.util.buf_versions[bufnr] = 37

local calls, cancelled = {}, 0
local function provider_client(path, id)
  return {
    id = id,
    name = "ktlsp",
    config = { root_dir = path, init_options = { analysisProviders = {
      ["mcdev.mixin"] = { command = { "companion", "--explicit" }, enabled = true,
        executeCommands = vim.deepcopy(commands) },
    } } },
    server_capabilities = { executeCommandProvider = { commands = { "ktlsp.providerCommand" } } },
    attached_buffers = { [bufnr] = true },
    request = function(self, method, params, callback, requested_bufnr)
      calls[#calls + 1] = { client = self.id, method = method, params = params, bufnr = requested_bufnr }
      callback(nil, { result = { OpaqueKey = "猫", CamelCase = { "A", "B" } } })
      return true, 41
    end,
    cancel_request = function(_, id)
      h.assert_eq(id, 41)
      cancelled = cancelled + 1
    end,
  }
end

local parent = provider_client(parent_root, 91)
local child = provider_client(root, 92)
vim.lsp.get_clients = function() return { parent, child } end
h.assert_eq(protocol.active_ktlsp_client(bufnr), child, "the deepest host owns this document")

for _, case in ipairs({
  { format = "unix", ending = "\n", eol = true },
  { format = "unix", ending = "\n", eol = false },
  { format = "dos", ending = "\r\n", eol = true },
  { format = "dos", ending = "\r\n", eol = false },
  { format = "mac", ending = "\r", eol = true },
}) do
  vim.bo[bufnr].fileformat = case.format
  vim.bo[bufnr].endofline = case.eol
  local text = table.concat({ "class 猫 {", "  // 🙂", "}" }, case.ending)
    .. (case.eol and case.ending or "")
  local context = protocol.context(bufnr, { 1, 1 })
  h.assert_eq(context.bufferText, text)
  h.assert_eq(context.bufferText, vim.lsp._buf_get_full_text(bufnr))
  h.assert_eq(context.documentVersion, 37)
  h.assert_true(context.documentVersion ~= vim.api.nvim_buf_get_changedtick(bufnr))
  for _, command in ipairs(commands) do
    local payload = { context = context, OpaquePayload = { MixedCase = "猫", Nested = { 1, "x" } } }
    local result, err
    local cancel = protocol.request(command, payload, function(value, error) result, err = value, error end, bufnr)
    h.assert_nil(err)
    h.assert_eq(result.result.OpaqueKey, "猫")
    local call = calls[#calls]
    h.assert_eq(call.client, child.id)
    h.assert_eq(call.method, "workspace/executeCommand")
    h.assert_eq(call.params.command, "ktlsp.providerCommand")
    h.assert_eq(#call.params.arguments, 1)
    local outer = call.params.arguments[1]
    h.assert_eq(outer.provider, "mcdev.mixin")
    h.assert_eq(outer.command, command)
    h.assert_eq(outer.context.workspaceRoot, root)
    h.assert_eq(outer.context.documentUri, vim.uri_from_bufnr(bufnr))
    h.assert_eq(outer.context.bufferText, text)
    h.assert_eq(outer.context.documentVersion, 37)
    h.assert_eq(#outer.arguments, 1)
    h.assert_true(outer.arguments[1] == payload, "domain payload must remain opaque")
    local refresh = command == protocol.commands.reindex or command == protocol.commands.reload_project_context
    h.assert_eq(outer.refreshProject, refresh and true or nil)
    cancel()
    cancel()
  end
end
h.assert_eq(cancelled, 5 * #commands, "cancellation is forwarded exactly once")

-- Neovim's LSP version is optional and must not be replaced by changedtick.
vim.lsp.util.buf_versions[bufnr] = nil
local unknown_version = protocol.context(bufnr, { 1, 1 })
h.assert_nil(unknown_version.documentVersion)
protocol.request(protocol.commands.info, { context = unknown_version }, function(_, err) h.assert_nil(err) end, bufnr)
h.assert_nil(calls[#calls].params.arguments[1].context.documentVersion)
local count, stale_error = #calls, nil
unknown_version.bufferText = "stale text"
protocol.request(protocol.commands.info, { context = unknown_version }, function(_, err) stale_error = err end, bufnr)
h.assert_eq(#calls, count)
h.assert_true(stale_error:find("stale", 1, true) ~= nil)
unknown_version.bufferText = vim.NIL
unknown_version.bufferTextFallback = "stale fallback"
protocol.request(protocol.commands.info, { context = unknown_version }, function(_, err) stale_error = err end, bufnr)
h.assert_eq(#calls, count)
h.assert_true(stale_error:find("stale", 1, true) ~= nil)
local invented_version = protocol.context(bufnr, { 1, 1 })
invented_version.documentVersion = vim.api.nvim_buf_get_changedtick(bufnr)
protocol.request(protocol.commands.info, { context = invented_version }, function(_, err) stale_error = err end, bufnr)
h.assert_eq(#calls, count)
h.assert_true(stale_error:find("stale", 1, true) ~= nil)

local init = child.config.init_options.analysisProviders["mcdev.mixin"]
init.executeCommands = { protocol.commands.completion }
h.assert_true(protocol.command_supported(child, protocol.commands.completion))
h.assert_true(not protocol.command_supported(child, protocol.commands.diagnostics))
count = #calls
local denied
protocol.request(protocol.commands.diagnostics, {}, function(_, err) denied = err end, bufnr)
h.assert_eq(#calls, count, "a denied nested provider must not fall back to its parent")
h.assert_true(denied:find("advertised", 1, true) ~= nil)
init.enabled = false
h.assert_nil(protocol.active_ktlsp_client(bufnr), "disabled nested provider must not use parent")
h.assert_true(not protocol.command_supported(child, protocol.commands.completion))
child.settings = { analysisProviders = { ["mcdev.mixin"] = { configuration = { changed = true } } } }
h.assert_nil(protocol.active_ktlsp_client(bufnr), "unrelated settings must not clear an explicit disable")
child.settings.analysisProviders["mcdev.mixin"].enabled = true
h.assert_eq(protocol.active_ktlsp_client(bufnr), child, "an explicit enable can override a disable")
child.settings = nil
init.enabled, init.executeCommands = true, vim.deepcopy(commands)
child.settings = { analysisProviders = { ["mcdev.mixin"] = { enabled = false } } }
h.assert_nil(protocol.active_ktlsp_client(bufnr), "live provider disable is respected")
child.settings = { analysisProviders = { ["mcdev.mixin"] = { executeCommands = { protocol.commands.info } } } }
h.assert_true(protocol.command_supported(child, protocol.commands.info))
h.assert_true(not protocol.command_supported(child, protocol.commands.completion), "live allowlist cannot be widened")
child.settings = nil
child.server_capabilities.executeCommandProvider.commands = { "another.command" }
h.assert_nil(protocol.active_ktlsp_client(bufnr), "a registered provider needs host advertisement")
h.assert_true(not protocol.command_supported(child, protocol.commands.completion))
child.server_capabilities.executeCommandProvider.commands = { "ktlsp.providerCommand" }
child.config.init_options.analysisProviders["mcdev.mixin"] = nil
h.assert_nil(protocol.active_ktlsp_client(bufnr), "host advertisement alone is not provider registration")
child.config.init_options.analysisProviders["mcdev.mixin"] = init
vim.lsp.get_clients = function() return { { name = "ktlsp", config = { root_dir = root } } } end
h.assert_nil(protocol.active_ktlsp_client(bufnr), "client name alone is not a command capability")
vim.lsp.get_clients = function() return { parent, child } end
child.config.root_dir = vim.uri_from_fname(root)
h.assert_eq(protocol.active_ktlsp_client(bufnr), child, "workspace file URIs resolve to the same native root")
child.config.root_dir = root

local original_request = child.request
child.request = function(_, _, _, callback)
  callback({ code = -32800, message = "cancelled by host" }, nil)
  return true, 41
end
local returned_error
protocol.request(protocol.commands.info, {}, function(_, err) returned_error = err end, bufnr)
h.assert_eq(returned_error.code, -32800)
h.assert_eq(returned_error.message, "cancelled by host")
child.request = original_request

if package.config:sub(1, 1) == "\\" then
  local get_name, fs_root = vim.api.nvim_buf_get_name, vim.fs.root
  vim.api.nvim_buf_get_name = function(buffer)
    if buffer == bufnr then return "//mcdev-server/share/project/src/Example.java" end
    return get_name(buffer)
  end
  vim.fs.root = function() return nil end
  local unc_owner = provider_client("//mcdev-server/share/project", 95)
  local unc_sibling = provider_client("//mcdev-server/share/project-other", 94)
  vim.lsp.get_clients = function() return { unc_sibling, unc_owner } end
  h.assert_eq(protocol.active_ktlsp_client(bufnr), unc_owner, "UNC ownership respects component boundaries")
  vim.api.nvim_buf_get_name, vim.fs.root = get_name, fs_root
end

local unattached = provider_client(root, 93)
unattached.attached_buffers = {}
vim.lsp.get_clients = function() return { unattached } end
local attached = 0
vim.lsp.buf_attach_client = function(buffer, id)
  h.assert_eq(buffer, bufnr)
  h.assert_eq(id, 93)
  attached = attached + 1
  unattached.attached_buffers[buffer] = true
  vim.lsp.util.buf_versions[buffer] = 0
  return true
end
protocol.request(protocol.commands.info, {}, function(_, err) h.assert_nil(err) end, bufnr)
h.assert_eq(attached, 1)
h.assert_eq(calls[#calls].params.arguments[1].context.documentVersion, 0)

local config = { init_options = { analysisProviders = { ["mcdev.mixin"] = {
  command = { "manual" }, enabled = false, executeCommands = { protocol.commands.info },
} } } }
h.assert_eq(ktlsp.extend_config(config), config)
local manual = config.init_options.analysisProviders["mcdev.mixin"]
h.assert_eq(manual.enabled, false)
h.assert_eq(#manual.executeCommands, 1)
h.assert_eq(manual.executeCommands[1], protocol.commands.info)
local generated = assert(ktlsp.extend_config({}, { companion = { cmd = { "companion" } } }))
h.assert_eq(#generated.init_options.analysisProviders["mcdev.mixin"].executeCommands, 11)

vim.lsp.get_clients, vim.lsp.buf_attach_client = saved_clients, saved_attach
vim.lsp.util.buf_versions[bufnr] = saved_version
vim.api.nvim_buf_delete(bufnr, { force = true })
print("mcdev-nvim ktlsp provider command routing tests passed")
