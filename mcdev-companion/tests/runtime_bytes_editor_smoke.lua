-- Test provider proof only: this does not invoke product Sugar/Local completion.
local project, artifacts = assert(vim.env.MCDEV_RUNTIME_PROJECT), assert(vim.env.MCDEV_RUNTIME_ARTIFACTS)
local host, probe = assert(vim.env.MCDEV_RUNTIME_HOST), assert(vim.env.MCDEV_RUNTIME_PROBE)
local provider = 'mcdev.runtime-bytes-probe'
local proof = { testOnly = true, checks = 0, assertions = {}, indexing = { messages = {} },
  productLocalCompletionVerified = false, fullSugarParityVerified = false, editorPid = vim.uv.os_getpid() }
local client, exited, files = nil, nil, {}
local system = vim.system
local function save() vim.fn.writefile({ vim.json.encode(proof) }, artifacts .. '/editor-proof.json') end
local function check(label, passed, detail)
  proof.checks = proof.checks + 1
  proof.assertions[#proof.assertions + 1] = { label = label, passed = passed == true, detail = detail }
  if not passed then error(label .. ': ' .. vim.inspect(detail)) end
end
local ok, failure = xpcall(function()
  vim.lsp.set_log_level('debug')
  vim.system = function(cmd, opts, callback)
    if cmd[1] == host then
      for _, stream in ipairs({ 'stdout', 'stderr' }) do
        local original = opts[stream]
        local file = assert(io.open(artifacts .. '/host.' .. stream .. '.raw', 'wb'))
        files[#files + 1] = file
        opts[stream] = function(err, data)
          if data then file:write(data); file:flush() end
          if original then original(err, data) end
        end
      end
    end
    local process = system(cmd, opts, callback)
    if cmd[1] == host then proof.hostPid = process.pid end
    return process
  end
  local path = project .. '/src/main/java/fixture/Owner.java'
  local bufnr = vim.fn.bufadd(path)
  vim.fn.bufload(bufnr)
  vim.bo[bufnr].filetype = 'java'
  vim.api.nvim_set_current_buf(bufnr)
  local id = assert(vim.lsp.start({
    name = 'mcdev-runtime-bytes-proof', cmd = { host }, root_dir = project,
    init_options = { analysisProviders = { [provider] = {
      enabled = true, command = { probe }, executeCommands = { 'mcdev.info' }, configuration = {},
    } } },
    on_exit = function(code, signal) exited = { code = code, signal = signal } end,
    handlers = { ['window/logMessage'] = function(_, params)
      local message = params and params.message or ''
      proof.indexing.messages[#proof.indexing.messages + 1] = message
      if message:match('^ktlsp indexed %d+ project files') then proof.indexing.project = true end
      if message:match('^ktlsp indexed %d+ library files') then proof.indexing.libraries = true end
    end },
  }, { bufnr = bufnr }))
  check('host initializes within 120s', vim.wait(120000, function()
    client = vim.lsp.get_client_by_id(id)
    return client and client.initialized
  end, 20), id)
  check('host finishes project and library indexing within 120s', vim.wait(120000, function()
    return proof.indexing.project and proof.indexing.libraries
  end, 20), proof.indexing)
  local text, uri = vim.lsp._buf_get_full_text(bufnr), vim.uri_from_bufnr(bufnr)
  local version = vim.lsp.util.buf_versions[bufnr]
  local context = { protocolVersion = 1, workspaceRoot = vim.uri_from_fname(project), documentUri = uri,
    languageId = 'java', position = { line = 0, character = 0 }, bufferText = text, documentVersion = version }
  check('current source and LSP version are explicit', context.bufferText == text
    and type(version) == 'number' and context.documentVersion == version, context)
  local outer = { command = 'ktlsp.providerCommand', arguments = {{
    provider = provider, command = 'mcdev.info',
    context = { workspaceRoot = project, documentUri = uri, bufferText = text, documentVersion = version },
    arguments = {{ context = context }},
  }} }
  proof.request = outer
  local response, problem = client:request_sync('workspace/executeCommand', outer, 30000, bufnr)
  proof.response = response or vim.NIL
  check('one test-provider request succeeds within 30s', response ~= nil and not problem and not response.err,
    { response = response, error = problem })
  local result = response.result
  vim.fn.writefile({ vim.json.encode(result) }, artifacts .. '/probe-result.json')
  check('reply identifies the test-only probe', result.testOnly == true and result.probe == 'runtime-bytes-code-lvt', result)
  check('Unknown or header-only bytes fail the proof', result.error == nil, result.error)
  check('probe uses current host evaluation source', result.documentUri == uri
    and result.documentVersion == version and result.sourceSha256 == vim.fn.sha256(text), result)
  local ops = result.negotiated.supportedOps['runtime-class-bytes-v1'] or {}
  check('actual initialization negotiated both byte operations', vim.list_contains(ops, 'runtimeClass')
    and vim.list_contains(ops, 'classBytes'), ops)
  check('descriptor is runtime Code authority', result.runtime.authority == 'runtime-bytecode'
    and result.runtime.codeAvailable == true and result.runtime.runtimeOnly == true, result.runtime)
  check('Rust decoded the exact draw overload and real instructions', result.method.owner == 'com/example/target/SimpleTarget'
    and result.method.descriptor == '(Ljava/lang/String;FF)V' and result.method.instructionCount > 0, result.method)
  check('Rust decoded text String at LVT slot 1', result.method['local'].name == 'text'
    and result.method['local'].slot == 1 and result.method['local'].descriptor == 'Ljava/lang/String;', result.method['local'])
  check('Rust decoded String.length invocation', result.method.stringLengthInvoke.opcode == 182
    and result.method.stringLengthInvoke.descriptor == '()I', result.method.stringLengthInvoke)
  proof.probePid, proof.scope = result.probePid, result.scope
end, debug.traceback)
if client then
  client:stop()
  local clean = vim.wait(15000, function() return exited ~= nil and client:is_stopped() end, 20)
    and exited.code == 0 and exited.signal == 0
  proof.shutdown = { clean = clean, exit = exited }
  if not clean then client:stop(true); vim.wait(5000, function() return exited ~= nil end, 20) end
  if not clean then ok, failure = false, (failure or '') .. '\nhost did not shut down cleanly' end
end
vim.system = system
for _, file in ipairs(files) do file:close() end
proof.scopePassed, proof.error = ok, ok and vim.NIL or tostring(failure)
save()
if not ok then io.stderr:write(tostring(failure) .. '\n'); vim.cmd('cquit 1') end
print('runtime bytes test provider: Code/LVT checks=' .. proof.checks .. '; external byte equality still required')
vim.cmd('qa!')
