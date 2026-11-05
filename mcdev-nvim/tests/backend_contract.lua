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
local host = { id = 81, name = "custom-host", server_capabilities = { executeCommandProvider = { commands = { protocol.commands.info } } }, config = { init_options = {
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

local provider_buf = vim.api.nvim_create_buf(false, true)
local provider_root = vim.fs.normalize(vim.fn.getcwd())
local provider_name = provider_root .. "/build/mcdev-protocol-provider.java"
vim.api.nvim_buf_set_name(provider_buf, provider_name)
vim.bo[provider_buf].filetype = "java"
local provider_version = 7
local provider_responses, provider_mutation, provider_deferred
local provider_frames, provider_handlers, provider_cancelled
local active_provider

local function make_provider(id)
  local provider = {
    id = id,
    name = "ktlsp",
    attached_buffers = { [provider_buf] = true },
    server_capabilities = { executeCommandProvider = { commands = { "ktlsp.providerCommand" } } },
    config = {
      root_dir = provider_root,
      init_options = { analysisProviders = { ["mcdev.mixin"] = {
        command = { "companion" },
        executeCommands = { protocol.commands.definition, protocol.commands.info },
        enabled = true,
      } } },
    },
  }
  provider.request = function(self, method, params, handler, request_bufnr)
    h.assert_eq(method, "workspace/executeCommand")
    h.assert_eq(request_bufnr, provider_buf)
    provider_frames[#provider_frames + 1] = { params = params, client = self }
    provider_handlers[#provider_handlers + 1] = handler
    local index = #provider_frames
    local response = provider_responses and provider_responses[index]
    if not provider_deferred and response then
      if provider_mutation and index == 1 then provider_mutation() end
      handler(response.err, response.result, { client_id = self.id })
    end
    return true, index
  end
  provider.cancel_request = function(_, request_id)
    provider_cancelled[#provider_cancelled + 1] = request_id
  end
  return provider
end

local provider = make_provider(901)
local replacement_provider = make_provider(902)
local function reset_provider(client)
  client.config.root_dir = provider_root
  client.config.init_options.analysisProviders["mcdev.mixin"] = {
    command = { "companion" },
    executeCommands = { protocol.commands.definition, protocol.commands.info },
    enabled = true,
  }
  client.attached_buffers = { [provider_buf] = true }
  client.server_capabilities.executeCommandProvider.commands = { "ktlsp.providerCommand" }
end

vim.lsp.get_clients = function()
  return { active_provider }
end
vim.lsp.util.buf_versions[provider_buf] = provider_version

local stale_response = { err = { code = -32801, data = { providerCommandReason = "Stale" } } }
local function run_provider(command, responses, mutation, deferred)
  vim.api.nvim_buf_set_name(provider_buf, provider_name)
  vim.api.nvim_buf_set_lines(provider_buf, 0, -1, false, { "class Provider {}" })
  vim.lsp.util.buf_versions[provider_buf] = provider_version
  reset_provider(provider)
  reset_provider(replacement_provider)
  active_provider = provider
  provider_responses, provider_mutation, provider_deferred = responses, mutation, deferred
  provider_frames, provider_handlers, provider_cancelled = {}, {}, {}
  local state = {
    callback_count = 0,
    result = nil,
    err = nil,
    frames = provider_frames,
    handlers = provider_handlers,
    cancelled = provider_cancelled,
  }
  local cancel = protocol.request(command, { context = protocol.context(provider_buf, { 1, 0 }), marker = "original" }, function(received, received_err)
    state.callback_count = state.callback_count + 1
    state.result, state.err = received, received_err
  end, provider_buf)
  state.cancel = cancel
  return state
end

local state = run_provider(protocol.commands.definition, { stale_response, { result = { ok = true } } })
h.assert_eq(#state.frames, 2)
h.assert_eq(state.callback_count, 1)
h.assert_true(state.result.ok)
h.assert_nil(state.err)
h.assert_true(vim.deep_equal(state.frames[1].params.arguments[1].context, state.frames[2].params.arguments[1].context))
h.assert_eq(state.frames[2].params.arguments[1].arguments[1].marker, "original")

state = run_provider(protocol.commands.definition, { stale_response, stale_response })
h.assert_eq(#state.frames, 2)
h.assert_eq(state.callback_count, 1)
h.assert_eq(state.err.code, -32801)

for _, response in ipairs({
  { command = protocol.commands.info, response = stale_response },
  { command = protocol.commands.definition, response = { err = { code = -32801, data = { providerCommandReason = "Other" } } } },
  { command = protocol.commands.definition, response = { err = { code = -32800, data = { providerCommandReason = "Stale" } } } },
  { command = protocol.commands.definition, response = { err = { code = -32801, data = {} } } },
}) do
  state = run_provider(response.command, { response.response })
  h.assert_eq(#state.frames, 1)
  h.assert_eq(state.callback_count, 1)
end

for _, mutation in ipairs({
  function() vim.api.nvim_buf_set_lines(provider_buf, 0, -1, false, { "class Changed {}" }) end,
  function() vim.lsp.util.buf_versions[provider_buf] = provider_version + 1 end,
  function() active_provider = replacement_provider end,
  function() provider.config.init_options.analysisProviders["mcdev.mixin"].command[1] = "other" end,
  function() provider.config.init_options.analysisProviders["mcdev.mixin"].enabled = false end,
  function() provider.config.root_dir = provider_root .. "/other" end,
  function() vim.api.nvim_buf_set_name(provider_buf, provider_root .. "/build/other.java") end,
}) do
  state = run_provider(protocol.commands.definition, { stale_response }, mutation)
  h.assert_eq(#state.frames, 1)
  h.assert_eq(state.callback_count, 1)
end

state = run_provider(protocol.commands.definition, nil, nil, true)
state.handlers[1](stale_response.err, nil, { client_id = provider.id })
h.assert_eq(#state.frames, 2)
state.handlers[2](nil, { ok = true }, { client_id = provider.id })
state.handlers[2](nil, { duplicate = true }, { client_id = provider.id })
state.handlers[1](nil, { late = true }, { client_id = provider.id })
h.assert_eq(state.callback_count, 1)
h.assert_eq(#state.cancelled, 0)

state = run_provider(protocol.commands.definition, nil, nil, true)
state.handlers[1](stale_response.err, nil, { client_id = provider.id })
h.assert_eq(#state.frames, 2)
state.cancel()
h.assert_eq(state.cancelled[#state.cancelled], 2)
state.handlers[2](nil, { ok = true }, { client_id = provider.id })
state.handlers[1](nil, { late = true }, { client_id = provider.id })
h.assert_eq(state.callback_count, 0)

vim.lsp.util.buf_versions[provider_buf] = nil
vim.api.nvim_buf_delete(provider_buf, { force = true })
vim.lsp.get_clients, protocol.code_action = saved.clients, saved.request
vim.notify, vim.lsp.get_client_by_id = saved.notify, saved.get_client
vim.api.nvim_buf_delete(bufnr, { force = true })
print("mcdev native backend contract and stale edit tests passed")
