local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")
local action = require("mcdev.code_action")
local config = require("mcdev.config")
local ktlsp = require("mcdev.ktlsp")
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname() .. ".java")
vim.bo[bufnr].filetype = "kotlin"
local buffer = require("mcdev.buffer")
local kotlin_imports = { "import org.spongepowered.asm.mixin.injection.*" }
for _, source in ipairs({ '@Inject(method = "ti', '@At(target = "Lexample/Target;ti', '@Inject(method = ["ti' }) do
  local lines = vim.list_extend(vim.deepcopy(kotlin_imports), { source })
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  h.assert_true(buffer.is_mcdev_buffer(bufnr))
  h.assert_eq(buffer.effective_language_id(bufnr), "kotlin")
  h.assert_true(buffer.is_mcdev_completion_context_at(bufnr, { #lines, #source }), source)
end
vim.bo[bufnr].filetype = "java"
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class Before {}" })
local saved = { clients = vim.lsp.get_clients, request = protocol.code_action,
  notify = vim.notify, get_client = vim.lsp.get_client_by_id }
local calls, notices = 0, {}
vim.notify = function(message) notices[#notices + 1] = message end
local host = { id = 81, name = "custom-host", config = { init_options = {
  analysisProviders = { ["mcdev.mixin"] = {} },
} }, request = function() calls = calls + 1; return true, calls end }
vim.lsp.get_clients = function() return { host } end
h.assert_eq(protocol.active_ktlsp_client(bufnr), host)
local unavailable
protocol.request(protocol.commands.completion, {}, function(_, err) unavailable = err end, bufnr)
h.assert_eq(calls, 0)
h.assert_true(unavailable:find("has not advertised", 1, true) ~= nil)
for _, command in pairs(protocol.commands) do
  host.server_capabilities = { executeCommandProvider = { commands = { command } } }
  h.assert_true(protocol.command_supported(host, command))
  protocol.request(command, {}, function() end, bufnr)
end
h.assert_eq(calls, 11)

local pending
protocol.code_action = function(_, _, _, callback) pending = callback end
local actions
action.code_actions(bufnr, nil, {}, function(items) actions = items end)
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class After {}" })
pending({ result = { actions = { { title = "old fix", edits = {
  { documentUri = vim.uri_from_bufnr(bufnr), edits = { { range = {
    start = { line = 0, character = 6 }, ["end"] = { line = 0, character = 12 },
  }, newText = "Changed" } } },
} } } } }, nil)
h.assert_eq(#actions, 1)
action.apply(actions[1], bufnr)
h.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], "class After {}")
h.assert_true(notices[#notices]:find("source changed", 1, true) ~= nil)
local blink = require("mcdev.blink").source()
local snapshot = require("mcdev.buffer").snapshot(bufnr)
local applied, callbacks = 0, 0
local item = { _mcdev_snapshot = snapshot }
blink:execute({}, item, function() callbacks = callbacks + 1 end, function() applied = applied + 1 end)
h.assert_eq(applied, 1)
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class Newer {}" })
blink:execute({}, item, function() callbacks = callbacks + 1 end, function() applied = applied + 1 end)
h.assert_eq(applied, 1, "stale Blink edits must be rejected before default application")
h.assert_eq(callbacks, 2, "stale acceptance must still settle Blink's task")
h.assert_true(notices[#notices]:find("completion source changed", 1, true) ~= nil)
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class After {}" })

local resolve
local client = { offset_encoding = "utf-16", supports_method = function() return true end,
  request = function(_, method, _, cb) h.assert_eq(method, "codeAction/resolve"); resolve = cb; return true end }
vim.lsp.get_client_by_id = function() return client end
action.apply({ title = "resolve stale", _mcdev_client_id = 1 }, bufnr)
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class Latest {}" })
resolve(nil, { edit = { changes = { [vim.uri_from_bufnr(bufnr)] = { {
  range = { start = { line = 0, character = 6 }, ["end"] = { line = 0, character = 11 } }, newText = "Wrong",
} } } } })
h.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], "class Latest {}")
h.assert_true(notices[#notices]:find("source changed", 1, true) ~= nil)
local configured = vim.deepcopy(config.options.companion)
config.options.companion.cmd = "shell string is not argv"
h.assert_nil(ktlsp.companion_command())
local invalid, invalid_error = ktlsp.extend_config({}, { companion = { cmd = "shell command" } })
h.assert_nil(invalid)
h.assert_true(invalid_error:find("argv list", 1, true) ~= nil)
config.options.companion = configured
vim.lsp.get_clients, protocol.code_action = saved.clients, saved.request
vim.notify, vim.lsp.get_client_by_id = saved.notify, saved.get_client
vim.api.nvim_buf_delete(bufnr, { force = true })
print("mcdev native backend contract and stale edit tests passed")
