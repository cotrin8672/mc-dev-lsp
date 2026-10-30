local h = dofile(vim.fn.getcwd() .. '/mcdev-nvim/tests/test_helpers.lua')
local completion = require('mcdev.completion')
local protocol = require('mcdev.protocol')
local transport = require('mcdev.transport')
local stdio = require('mcdev.stdio')
local saved = { clients = vim.lsp.get_clients, request = transport.request, ready = transport.when_ready,
  generation = transport.ready_generation, stdio = stdio.request,
  error = completion.last_error, project_error = completion.last_project_transport_error }
local root = vim.fs.normalize(vim.fn.tempname())
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, root .. '/Example.java')
vim.bo[bufnr].filetype = 'java'
vim.lsp.util.buf_versions[bufnr] = 31
local selected, frames, helper_callback, delayed_helper, selections = nil, {}, nil, false, 0
local endpoint_error = 'mcdev: completion transport unavailable'
local helper = { result = { items = {{ label = 'helper', insertText = 'helper', kind = 'keyword' }} } }
local actual = { result = { items = {{ label = 'actual provider', insertText = 'actual', kind = 'keyword',
  metadata = { source = 'mixin.annotation' } }} } }
local cancel_current
local function make_client(provider)
  return {
    id = 300 + #frames, name = provider and 'ktlsp' or 'jdtls', initialized = true,
    config = { root_dir = root, init_options = { analysisProviders = { ['mcdev.mixin'] = {
      command = { 'companion' }, enabled = true, executeCommands = { protocol.commands.completion },
    } } } },
    server_capabilities = { executeCommandProvider = {
      commands = provider and { 'ktlsp.providerCommand' } or { protocol.commands.completion },
    } },
    attached_buffers = { [bufnr] = true },
    request = function(self, method, params, callback, buffer)
      frames[#frames + 1] = { client = self, method = method, params = params, callback = callback, bufnr = buffer }
      return true, #frames
    end,
    cancel_request = function() end,
  }
end
local function begin(provider, delay_helper)
  selected, delayed_helper = make_client(provider), delay_helper
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { '@In' })
  local results = {}
  cancel_current = completion.complete(function(result) results[#results + 1] = result end, bufnr, { 1, 3 })
  local frame = frames[#frames]
  h.assert_eq(frame.bufnr, bufnr)
  h.assert_eq(frame.method, 'workspace/executeCommand')
  h.assert_eq(frame.params.command, provider and 'ktlsp.providerCommand' or protocol.commands.completion)
  h.assert_eq(completion.last_project_transport_error, endpoint_error)
  return frame, results, cancel_current
end
local function reply(frame, result, err, client_id)
  local before = selections
  frame.callback(err, result, { client_id = client_id or frame.client.id })
  h.assert_eq(selections, before, 'RPC delivery must use dispatch origin without reselecting a client')
end
local ok, failure = xpcall(function()
  vim.lsp.get_clients = function() selections = selections + 1; return selected and { selected } or {} end
  transport.request = function() return nil, endpoint_error end
  transport.when_ready = function() return nil end
  transport.ready_generation = function() return 0 end
  stdio.request = function(_, callback)
    helper_callback = callback
    if not delayed_helper then callback(vim.deepcopy(helper), nil) end
    return function() end
  end

  local frame, results = begin(true)
  reply(frame, vim.deepcopy(actual))
  h.assert_nil(completion.last_project_transport_error, 'a valid dispatched provider completion replaces the unavailable loopback')
  h.assert_nil(completion.last_error)
  h.assert_eq(results[1].items[1].insertText, 'actual', 'the actual host item must be used')
  h.assert_eq(results[1].items[1].data.source, 'mixin.annotation')

  frame, results = begin(true)
  reply(frame, { result = { items = {} } })
  h.assert_nil(completion.last_project_transport_error, 'a successful empty provider list is also a valid completion')

  frame, results = begin(true, true)
  reply(frame, nil, { code = -32002, message = 'provider failed' })
  h.assert_eq(#results, 0)
  h.assert_not_nil(completion.last_error)
  helper_callback(vim.deepcopy(helper), nil)
  h.assert_eq(results[1].items[1].insertText, 'helper')
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'stdio fallback cannot erase the transport failure')

  for _, case in ipairs({
    { name = 'typed incomplete', envelope = { error = { code = 'INCOMPLETE_PROJECT_CONTEXT', message = 'not ready' } } },
    { name = 'missing result', envelope = {} },
    { name = 'null result', envelope = { result = vim.NIL } },
    { name = 'scalar result', envelope = { result = 17 } },
    { name = 'missing items', envelope = { result = {} } },
    { name = 'scalar items', envelope = { result = { items = 'bad' } } },
    { name = 'object items', envelope = { result = { items = { unexpected = true } } } },
    { name = 'invalid item', envelope = { result = { items = { 17 } } } },
    { name = 'missing label', envelope = { result = { items = {{ insertText = 'bad' }} } } },
    { name = 'invalid incomplete flag', envelope = { result = { items = {}, isIncomplete = 'bad' } } },
    { name = 'scalar envelope', envelope = 17 },
    { name = 'malformed domain error', envelope = { error = true } },
  }) do
    frame = begin(true)
    reply(frame, case.envelope)
    h.assert_eq(completion.last_project_transport_error, endpoint_error, case.name .. ' cannot clear the transport failure')
  end

  frame = begin(false)
  frame.client.server_capabilities.executeCommandProvider.commands = { 'ktlsp.providerCommand' }
  reply(frame, vim.deepcopy(actual))
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'changing config cannot upgrade a dispatched legacy route')

  frame = begin(false)
  selected = make_client(true)
  reply(frame, vim.deepcopy(actual))
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'selecting another provider cannot upgrade a foreign legacy reply')

  frame = begin(true)
  reply(frame, vim.deepcopy(actual), nil, frame.client.id + 1000)
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'a foreign RPC client context cannot authenticate provider success')

  frame = begin(true)
  frame.client.server_capabilities.executeCommandProvider.commands = { protocol.commands.completion }
  frame.client.config.init_options.analysisProviders['mcdev.mixin'].enabled = false
  selected = make_client(false)
  reply(frame, vim.deepcopy(actual))
  h.assert_nil(completion.last_project_transport_error, 'a genuinely dispatched provider response keeps its route despite later config changes')

  local cancel
  frame, results, cancel = begin(true)
  cancel()
  reply(frame, vim.deepcopy(actual))
  h.assert_eq(#results, 0)
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'a cancelled response cannot clear the failure')

  frame, results = begin(true)
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { '@In changed' })
  reply(frame, vim.deepcopy(actual))
  h.assert_true(results[1].isStale)
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'a stale response cannot clear the failure')

  local prior = begin(true)
  endpoint_error = 'new request transport failure'
  frame, results, cancel = begin(false)
  reply(prior, vim.deepcopy(actual))
  h.assert_eq(#results, 0)
  h.assert_eq(completion.last_project_transport_error, endpoint_error, 'a superseded response cannot erase the current request error')
  cancel()

  selected = make_client(true)
  local received, received_error, origin
  protocol.completion(function(value, err, context) received, received_error, origin = value, err, context end, bufnr, { 1, 0 })
  frame = frames[#frames]
  reply(frame, actual)
  h.assert_eq(received, actual)
  h.assert_nil(received_error)
  h.assert_eq(origin.client, frame.client)
  h.assert_eq(origin.client_id, frame.client.id)
  h.assert_eq(origin.route, 'ktlsp.providerCommand')
  h.assert_eq(origin.provider, 'mcdev.mixin')
  h.assert_eq(origin.command, protocol.commands.completion)
  protocol.completion(function(value, err) received, received_error = value, err end, bufnr, { 1, 0 })
  reply(frames[#frames], helper)
  h.assert_eq(received, helper, 'existing two-argument native callbacks still receive their original result')
  h.assert_nil(received_error)
end, debug.traceback)
if cancel_current then cancel_current() end
vim.lsp.get_clients, transport.request, transport.when_ready = saved.clients, saved.request, saved.ready
transport.ready_generation, stdio.request = saved.generation, saved.stdio
completion.last_error, completion.last_project_transport_error = saved.error, saved.project_error
vim.lsp.util.buf_versions[bufnr] = nil
vim.api.nvim_buf_delete(bufnr, { force = true })
if not ok then error(failure) end
print('mcdev-nvim provider completion origin and transport-error regressions passed')
