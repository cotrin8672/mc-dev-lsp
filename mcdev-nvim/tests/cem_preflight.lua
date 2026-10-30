local repo = vim.fn.getcwd()
local h = dofile(repo .. '/mcdev-nvim/tests/test_helpers.lua')
local ktlsp = require('mcdev.ktlsp')
local stdio = require('mcdev.stdio')
local transport = require('mcdev.transport')
local saved = { start = ktlsp.start_or_attach, clients = vim.lsp.get_clients,
  client = vim.lsp.get_client_by_id, wait = vim.wait, time = vim.uv.hrtime,
  write = vim.fn.writefile, dofile = dofile, record = _G.mcdev_record_assertion,
  stdio_stop = stdio.stop, transport_stop = transport.stop,
  current = vim.api.nvim_get_current_buf(), options = vim.deepcopy(require('mcdev.config').options) }
local environment = { 'MCDEV_SNIPPET_PROJECT', 'MCDEV_SNIPPET_ARTIFACTS', 'MCDEV_SNIPPET_HOST', 'MCDEV_SNIPPET_COMPANION' }
for _, name in ipairs(environment) do saved[name] = vim.env[name] end
local project = vim.fs.normalize(repo .. '/build/nvim-test-logs/cem-preflight-unit-' .. vim.fn.getpid())
local path = project .. '/src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java'
local artifacts = project .. '/artifacts'
vim.fn.mkdir(vim.fs.dirname(path), 'p')
vim.fn.mkdir(artifacts, 'p')
vim.fn.writefile({ '// unchanged saved source', 'class ItemMixin {}' }, path)
local bufnr, client, proof, reached, now, pauses, calls, cancellations, scenario
local unavailable = { code = -32002, message = 'Server error', data = { providerCommandReason = 'CommandUnavailable' } }
local ok, failure = xpcall(function()
  vim.env.MCDEV_SNIPPET_PROJECT, vim.env.MCDEV_SNIPPET_ARTIFACTS = project, artifacts
  vim.env.MCDEV_SNIPPET_HOST, vim.env.MCDEV_SNIPPET_COMPANION = vim.v.progpath, vim.v.progpath
  ktlsp.start_or_attach = function(config)
    bufnr = vim.api.nvim_get_current_buf()
    vim.lsp.util.buf_versions[bufnr] = 31
    client = { id = 900, initialized = true, config = assert(ktlsp.extend_config(config)),
      attached_buffers = { [bufnr] = true },
      server_capabilities = { executeCommandProvider = { commands = { 'ktlsp.providerCommand' } } },
      request = function(_, method, params, callback, buffer)
        h.assert_eq(method, 'workspace/executeCommand')
        h.assert_eq(params.command, 'ktlsp.providerCommand')
        h.assert_eq(params.arguments[1].command, 'mcdev.info')
        h.assert_eq(buffer, bufnr)
        calls = calls + 1
        local response = scenario.replies[math.min(calls, #scenario.replies)]
        if not response.timeout then
          callback(response.err, response.value, { client_id = response.foreign and 901 or 900 })
        end
        return true, calls
      end,
      cancel_request = function() cancellations = cancellations + 1 end,
      stop = function() config.on_exit(0, 0) end,
    }
    return client.id
  end
  vim.lsp.get_clients = function() return client and { client } or {} end
  vim.lsp.get_client_by_id = function(id) return client and client.id == id and client or nil end
  vim.uv.hrtime = function() return now * 1e6 end
  vim.wait = function(ms, callback)
    if callback() then return true end
    pauses[#pauses + 1] = ms
    now = now + ms
    return callback() == true
  end
  stdio.stop, transport.stop = function() end, function() end
  _G.dofile = function(file)
    if file == repo .. '/mcdev-nvim/tests/e2e/cem_feature_snippets.lua' then
      return { inject_lines = {}, run = function()
        reached = true
        error('preflight reached the untouched snippet group')
      end }
    end
    return saved.dofile(file)
  end
  vim.fn.writefile = function(lines, file, flags)
    if file == artifacts .. '/editor-proof.json' then
      proof = vim.json.decode(lines[1])
      return 0 -- Keep mocked entry receipts in memory; they are never live-host evidence.
    end
    return saved.write(lines, file, flags or '')
  end
  for _, case in ipairs({
    { name = 'bounded retry', ready = true, count = 3, replies = {
      { err = unavailable }, { err = unavailable }, { value = { result = { lines = { 'ready' } } } },
    } },
    { name = 'typed project incomplete', ready = true, count = 1, replies = {
      { value = { error = { code = 'INCOMPLETE_PROJECT_CONTEXT', message = 'indexing' } } },
    } },
    { name = 'ordinary RPC failure', count = 1, replies = { { err = { code = -32603, message = 'failed' } } } },
    { name = 'other unavailable reason', count = 1, replies = {
      { err = { code = -32002, data = { providerCommandReason = 'OtherFailure' } } },
    } },
    { name = 'domain failure', count = 1, replies = { { value = { error = { code = 'INVALID_ARGUMENT', message = 'bad' } } } } },
    { name = 'missing result', count = 1, replies = { { value = {} } } },
    { name = 'foreign client', count = 1, replies = { { value = { result = {} }, foreign = true } } },
    { name = 'request timeout', count = 1, replies = { { timeout = true } } },
    { name = 'overall deadline', replies = { { err = unavailable } } },
  }) do
    scenario, proof, reached, now, pauses, calls, cancellations, client = case, nil, false, 0, {}, 0, 0, nil
    _G.mcdev_record_assertion = function() end -- The entry's initial environment asserts are mocked too.
    local passed = pcall(saved.dofile, repo .. '/mcdev-nvim/tests/e2e/cem_snippets_host.lua')
    _G.mcdev_record_assertion = saved.record
    h.assert_true(not passed, 'the mock must stop before any original snippet group runs')
    h.assert_not_nil(proof, case.name .. ' must leave a bounded failure/readiness receipt')
    h.assert_eq(proof.preflight.ready, case.ready == true, case.name)
    h.assert_eq(reached, case.ready == true, case.name .. ' cannot edit snippet inputs before real readiness')
    h.assert_eq(#proof.frames, 0, 'preflight info requests are separate from the four completion frames')
    h.assert_eq(#proof.snippets, 0, 'mock readiness cannot credit original snippet assertions')
    h.assert_true(proof.diskUnchanged and proof.shutdown.clean)
    h.assert_true(now <= 60000, 'preflight must have a finite overall deadline')
    if case.count then h.assert_eq(calls, case.count, case.name .. ' retry count') end
    if case.name == 'bounded retry' then
      h.assert_eq(pauses[1], 100)
      h.assert_eq(pauses[2], 200)
      h.assert_eq(proof.preflight.attempts[1].retry, 'CommandUnavailable')
    elseif case.name == 'typed project incomplete' then
      h.assert_true(proof.preflight.attempts[1].typedProjectIncomplete)
    elseif case.name == 'request timeout' then
      h.assert_eq(cancellations, 1)
    elseif case.name == 'overall deadline' then
      h.assert_eq(now, 60000)
      h.assert_true(calls > 1)
    end
  end
  h.assert_eq(vim.fn.filereadable(artifacts .. '/editor-proof.json'), 0, 'no mocked live-host proof may be written')
end, debug.traceback)
ktlsp.start_or_attach, vim.lsp.get_clients, vim.lsp.get_client_by_id = saved.start, saved.clients, saved.client
vim.wait, vim.uv.hrtime, vim.fn.writefile = saved.wait, saved.time, saved.write
_G.dofile, _G.mcdev_record_assertion = saved.dofile, saved.record
stdio.stop, transport.stop = saved.stdio_stop, saved.transport_stop
require('mcdev.config').options = saved.options
for _, name in ipairs(environment) do vim.env[name] = saved[name] end
vim.api.nvim_set_current_buf(saved.current)
if bufnr then vim.lsp.util.buf_versions[bufnr] = nil; vim.api.nvim_buf_delete(bufnr, { force = true }) end
if not ok then error(failure) end
print('CEM provider preflight retry, failure, deadline and evidence-isolation unit tests passed')
