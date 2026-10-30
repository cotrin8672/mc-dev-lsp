-- Real provider responses and the shared original CEM expansion group; no Minecraft type lookup is required.
local repo = vim.fn.getcwd()
local project = assert(vim.env.MCDEV_SNIPPET_PROJECT, 'missing isolated snippet project')
local artifacts = assert(vim.env.MCDEV_SNIPPET_ARTIFACTS, 'missing snippet artifact path')
local binary = assert(vim.env.MCDEV_SNIPPET_HOST, 'missing pinned host binary')
local companion = assert(vim.env.MCDEV_SNIPPET_COMPANION, 'missing pinned companion binary')
local helpers = dofile(repo .. '/mcdev-nvim/tests/test_helpers.lua')
local group = dofile(repo .. '/mcdev-nvim/tests/e2e/cem_feature_snippets.lua')
local protocol = require('mcdev.protocol')
local completion = require('mcdev.completion')
local path = project .. '/src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java'
local disk_lines = vim.fn.readfile(path)
local disk_source = table.concat(disk_lines, '\n')
local bufnr = vim.fn.bufadd(path)
vim.fn.bufload(bufnr)
vim.api.nvim_set_current_buf(bufnr)
vim.bo[bufnr].filetype = 'java'
local proof = {
  mode = 'real-host', scope = 'original CEM Inject, Implements, Descriptors, Group snippets',
  scopePassed = false, fullCemWorkflowVerified = false,
  unverified = { 'Item/BlockItem target members', 'Shadow fields and methods', 'Blink cache and aggregate routing' },
  host = binary, companion = companion, document = path, diskBefore = disk_source,
  assertions = {}, snippets = {}, frames = {}, messages = {}, shutdown = { exited = false },
  preflight = { command = 'mcdev.info', deadlineMs = 60000, attempts = {}, ready = false,
    role = 'actual provider transport readiness; no snippet evidence' },
}
_G.mcdev_record_assertion = function(kind, caller)
  proof.assertions[#proof.assertions + 1] = {
    kind = kind, source = (caller.source or ''):gsub('^@', ''):gsub('\\', '/'), line = caller.currentline,
  }
end
local client, client_id
local support = { helpers = helpers, mixin_file = path,
  log_step = function(message) vim.fn.writefile({ message }, artifacts .. '/progress.log', 'a') end }
local ok, failure = xpcall(function()
  require('mcdev').setup({ ktlsp = { cmd = { binary } }, companion = { cmd = { companion } } })
  client_id = assert(require('mcdev.ktlsp').start_or_attach({
    root_dir = project, flags = { debounce_text_changes = 0 },
    on_exit = function(code, signal)
      proof.shutdown.exited, proof.shutdown.exitCode, proof.shutdown.signal = true, code, signal
    end,
    handlers = { ['window/logMessage'] = function(_, params)
      proof.messages[#proof.messages + 1] = params and params.message or ''
    end },
  }))
  helpers.assert_true(vim.wait(30000, function()
    client = vim.lsp.get_client_by_id(client_id)
    return client and client.initialized and client.attached_buffers[bufnr]
  end, 25), 'pinned host must initialize and attach within 30s')
  helpers.assert_true(protocol.command_supported(client, protocol.commands.completion),
    'host must expose the provider-command route; actual responses are checked separately')
  local original_request = client.request
  client.request = function(self, method, params, callback, requested_bufnr)
    if method ~= 'workspace/executeCommand' then
      return original_request(self, method, params, callback, requested_bufnr)
    end
    local outer = params.arguments and params.arguments[1]
    if type(outer) ~= 'table' or outer.command ~= protocol.commands.completion then
      return original_request(self, method, params, callback, requested_bufnr)
    end
    local frame = {
      params = vim.deepcopy(params), bufnr = requested_bufnr,
      text = vim.lsp._buf_get_full_text(bufnr), version = vim.lsp.util.buf_versions[bufnr],
    }
    proof.frames[#proof.frames + 1] = frame
    return original_request(self, method, params, function(err, result, ...)
      frame.error, frame.result, frame.replied = err, vim.deepcopy(result), true
      if callback then return callback(err, result, ...) end
    end, requested_bufnr)
  end
  local readiness_started = vim.uv.hrtime()
  local backoff_ms = 100
  local function remaining_ms()
    return math.max(0, math.floor(proof.preflight.deadlineMs - (vim.uv.hrtime() - readiness_started) / 1e6))
  end
  while remaining_ms() > 0 do
    local attempt = {}
    proof.preflight.attempts[#proof.preflight.attempts + 1] = attempt
    local response_origin
    local cancel = protocol.request(protocol.commands.info, { context = protocol.context(bufnr, { 1, 0 }) },
      function(result, err, origin)
        attempt.result, attempt.error, attempt.replied = result, err, true
        response_origin = origin
        attempt.origin = origin and { clientId = origin.client_id, route = origin.route,
          command = origin.command, provider = origin.provider } or nil
      end, bufnr)
    local replied = vim.wait(math.min(5000, remaining_ms()), function() return attempt.replied end, 25)
    if not replied then cancel() end
    helpers.assert_true(replied, 'provider readiness probe timed out')
    helpers.assert_true(response_origin and response_origin.client == client
      and response_origin.route == 'ktlsp.providerCommand' and response_origin.provider == 'mcdev.mixin'
      and response_origin.command == protocol.commands.info, 'readiness must come from the dispatched provider')
    local err = attempt.error
    local unavailable = type(err) == 'table' and err.code == -32002
      and type(err.data) == 'table' and err.data.providerCommandReason == 'CommandUnavailable'
    if not unavailable then
      helpers.assert_nil(err, 'provider readiness RPC failed: ' .. vim.inspect(err))
      local result = attempt.result
      helpers.assert_true(type(result) == 'table', 'provider readiness needs an actual response envelope')
      local incomplete = type(result.error) == 'table' and type(result.error.code) == 'string'
        and result.error.code:upper() == 'INCOMPLETE_PROJECT_CONTEXT'
      if not incomplete then
        helpers.assert_nil(result.error, 'provider readiness command failed: ' .. vim.inspect(result.error))
        helpers.assert_true(type(result.result) == 'table', 'provider readiness result is missing or malformed')
      end
      attempt.typedProjectIncomplete, proof.preflight.ready = incomplete, true
      break
    end
    attempt.retry = 'CommandUnavailable'
    local pause = math.min(backoff_ms, remaining_ms())
    if pause > 0 then vim.wait(pause, function() return false end, 25) end
    backoff_ms = math.min(backoff_ms * 2, 1000)
  end
  helpers.assert_true(proof.preflight.ready, 'provider did not acknowledge readiness within 60s')
  group.run(support, bufnr, group.inject_lines, 7, disk_source, proof.snippets)
  helpers.assert_eq(#proof.snippets, 4, 'all four original snippet cases must finish')
  helpers.assert_eq(#proof.frames, 4, 'each original snippet needs its own real host response')
  for index, snippet in ipairs(proof.snippets) do
    local frame = proof.frames[index]
    helpers.assert_true(frame.replied, snippet.label .. ' host response must complete')
    helpers.assert_nil(frame.error, snippet.label .. ' host request failed')
    helpers.assert_not_nil(frame.result, snippet.label .. ' host returned no envelope')
    helpers.assert_nil(frame.result.error, snippet.label .. ' host returned a domain error')
    helpers.assert_eq(frame.params.command, 'ktlsp.providerCommand')
    local outer = frame.params.arguments[1]
    helpers.assert_eq(outer.provider, 'mcdev.mixin')
    helpers.assert_eq(frame.bufnr, bufnr)
    helpers.assert_eq(outer.context.documentUri, vim.uri_from_bufnr(bufnr))
    helpers.assert_eq(outer.context.bufferText, frame.text)
    helpers.assert_true(type(frame.version) == 'number', 'the attached document needs its actual LSP version')
    helpers.assert_eq(outer.context.documentVersion, frame.version)
    helpers.assert_eq(table.concat(snippet.input, '\n'), frame.text:gsub('\r\n', '\n'):gsub('\n$', ''))
    local actual = vim.iter((frame.result.result or {}).items or {}):find(function(item)
      return item.label == snippet.label
    end)
    helpers.assert_not_nil(actual, snippet.label .. ' must be present in the actual host response')
    helpers.assert_true(vim.deep_equal(completion.to_lsp_item(actual), snippet.item),
      snippet.label .. ' expanded item must equal the host response, including filter and every edit')
    frame.sourceSha256 = vim.fn.sha256(frame.text)
  end
  proof.scopePassed = true
end, debug.traceback)
vim.snippet.stop()
require('mcdev.stdio').stop('snippet E2E cleanup')
require('mcdev.transport').stop()
client = client or (client_id and vim.lsp.get_client_by_id(client_id))
if client then
  client:stop(false)
  proof.shutdown.clean = vim.wait(15000, function() return proof.shutdown.exited end, 25)
    and proof.shutdown.exitCode == 0 and proof.shutdown.signal == 0
  if not proof.shutdown.clean then
    client:stop(true)
    vim.wait(3000, function() return proof.shutdown.exited end, 25)
  end
end
proof.diskAfter = table.concat(vim.fn.readfile(path), '\n')
proof.diskUnchanged = proof.diskAfter == disk_source
proof.error = not ok and failure or nil
proof.scopePassed = proof.scopePassed and proof.shutdown.clean == true and proof.diskUnchanged
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, disk_lines)
vim.bo[bufnr].modified = false
vim.fn.writefile({ vim.json.encode(proof) }, artifacts .. '/editor-proof.json')
helpers.assert_true(ok, failure)
helpers.assert_true(proof.scopePassed, 'all four original snippets, unchanged disk, and clean owned-host shutdown are required')
print('four original CEM snippet expansions passed through the real host; member/Shadow/Blink workflows remain unverified')
