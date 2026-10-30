-- Real ktlsp + mcdev companion commands against an isolated Gradle project.
local project = assert(vim.env.MCDEV_COMMAND_PROJECT)
local artifacts = assert(vim.env.MCDEV_COMMAND_ARTIFACTS)
local binary = assert(vim.env.MCDEV_COMMAND_HOST)
local companion = assert(vim.env.MCDEV_COMMAND_COMPANION)
local repo = vim.fn.getcwd()
vim.lsp.set_log_level('debug')
vim.env.KTLSP_TRACE = artifacts .. '/host.trace.jsonl'
local acceptance_module = dofile(repo .. '/mcdev-companion/tests/support/accepted_trace.lua')
local acceptance = acceptance_module.start(vim.env.KTLSP_TRACE, 'mcdev.mixin')
local protocol = require('mcdev.protocol')
local provider = 'mcdev.mixin'
local command_ids = {
  'mcdev.completion', 'mcdev.diagnostics', 'mcdev.context', 'mcdev.definition',
  'mcdev.hover', 'mcdev.references', 'mcdev.codeAction', 'mcdev.reindex',
  'mcdev.reloadProjectContext', 'mcdev.dumpContext', 'mcdev.info',
}
local incomplete_ids = {
  'mcdev.references', 'mcdev.codeAction', 'mcdev.reindex',
  'mcdev.reloadProjectContext', 'mcdev.dumpContext', 'mcdev.info',
}
local proof = {
  checks = 0, assertions = {}, matrix = {}, requests = {}, frames = {},
  indexing = {}, shutdown = {}, hostPids = {}, childProcesses = {},
  editorPid = vim.uv.os_getpid(), gradleBuildSucceeded = vim.env.MCDEV_COMMAND_GRADLE_BUILD_OK == '1',
  completeReplacementVerified = false, sdkSourceFixture = 'reduced Object.java',
}
local client, buffers, exited = nil, {}, nil
local host_system, host_debug = vim.system, vim.lsp.log.debug
local cancel_id, cancel_wire
local paths = {
  owner = project .. '/src/main/java/fixture/Owner.java',
  patch = project .. '/src/main/java/fixture/Patch.java',
  expression = project .. '/src/main/java/fixture/ExampleMixin.java',
  target = project .. '/src/main/java/fixture/Target.java',
  kotlin = project .. '/src/main/java/fixture/NoActions.kt',
  aw = project .. '/src/main/resources/access.aw',
}
local function saved(value) return value == nil and vim.NIL or vim.deepcopy(value) end
local function save()
  vim.fn.writefile({ vim.json.encode(proof) }, artifacts .. '/editor-proof.json')
  local lines = {}
  for _, frame in ipairs(proof.frames) do lines[#lines + 1] = vim.json.encode(frame) end
  vim.fn.writefile(lines, artifacts .. '/client-frames.jsonl')
end
local function check(label, condition, detail)
  proof.checks = proof.checks + 1
  proof.assertions[#proof.assertions + 1] = { label = label, passed = condition == true, detail = saved(detail) }
  if not condition then error(label .. ': ' .. vim.inspect(detail)) end
end
local function check_phase(label, condition, detail)
  if not condition then proof.phaseFailure = { phase = label, detail = saved(detail) } end
  check(label, condition, detail)
end
local function run_case(label, command, body)
  local ok, value = xpcall(body, debug.traceback)
  proof.matrix[#proof.matrix + 1] = {
    case = label, command = command, status = ok and 'passed' or 'failed',
    featureProved = ok and command ~= nil or false, detail = ok and saved(value) or tostring(value),
  }
  save()
  if ok then print('PASS ' .. label) else print('FAIL ' .. label .. ': ' .. tostring(value)) end
  if not ok and proof.phaseFailure then error(value, 0) end
  return ok, value
end
local function provider_settings(enabled)
  return { enabled = enabled, command = { companion }, executeCommands = command_ids, configuration = {} }
end
local function full_text(bufnr)
  return vim.lsp._buf_get_full_text(bufnr)
end
local function flush()
  require('vim.lsp._changetracking').flush(client)
end
local function uri(bufnr) return vim.uri_from_bufnr(bufnr) end
local function await_accepted(bufnr, floor, options)
  local expected = {}
  expected[acceptance_module.owner_key(vim.uri_to_fname(uri(bufnr)))] = vim.fn.sha256(full_text(bufnr))
  local waited, ok, found = pcall(acceptance.wait_for, acceptance, expected, floor, 30000, options or {})
  check_phase('host accepted current source hash', waited and ok,
    { expected = expected, found = found, error = not waited and tostring(ok) or nil, deadlineMs = 30000 })
  return found
end
local function observe_client(current)
  local original = current.request
  current.request = function(self, method, params, callback, bufnr)
    if method ~= 'workspace/executeCommand' then return original(self, method, params, callback, bufnr) end
    local frame = { method = method, params = vim.deepcopy(params), at = vim.uv.hrtime() }
    proof.frames[#proof.frames + 1] = frame
    local function wrapped(err, result, ...)
      frame.error, frame.result = saved(err), saved(result)
      frame.responseAt = vim.uv.hrtime()
      if callback then return callback(err, result, ...) end
    end
    local sent, id = original(self, method, params, wrapped, bufnr)
    frame.requestId = id
    return sent, id
  end
end
local function start_server()
  local indexing = { project = false, libraries = false, messages = {}, deadlineMs = 180000 }
  local server_exit = { exited = false }
  exited = server_exit
  local id = assert(vim.lsp.start({
    name = 'mcdev-command-proof', cmd = { binary }, root_dir = project,
    init_options = { analysisProviders = { [provider] = provider_settings(true) } },
    on_exit = function(code, signal)
      server_exit.exited, server_exit.exit_code, server_exit.signal = true, code, signal
    end,
    handlers = { ['window/logMessage'] = function(_, params)
      local message = params and params.message or ''
      if message:match('^ktlsp indexed %d+ project files') then indexing.project = true end
      if message:match('^ktlsp indexed %d+ library files') then indexing.libraries = true end
      indexing.messages[#indexing.messages + 1] = message
    end },
  }, { bufnr = buffers.owner }))
  check_phase('LSP initializes', vim.wait(30000, function()
    client = vim.lsp.get_client_by_id(id)
    return client ~= nil and client.initialized
  end, 20), id)
  observe_client(client)
  for _, bufnr in pairs(buffers) do vim.lsp.buf_attach_client(bufnr, id) end
  local indexed = vim.wait(indexing.deadlineMs, function() return indexing.project and indexing.libraries end, 20)
  proof.indexing[#proof.indexing + 1] = indexing
  if not indexed then proof.indexingIncomplete = true end
  check_phase('host finished project and library indexing', indexed, indexing)
  local commands = client.server_capabilities.executeCommandProvider
  check('host advertises providerCommand', commands and vim.list_contains(commands.commands or {}, 'ktlsp.providerCommand'), commands)
  return server_exit
end
local function stop_server(exited)
  if not client then return end
  client:stop()
  exited.clean = vim.wait(15000, function() return exited.exited and client:is_stopped() end, 20)
    and exited.exit_code == 0 and exited.signal == 0
  if not exited.clean then
    client:stop(true)
    vim.wait(5000, function() return exited.exited end, 20)
  end
  proof.shutdown[#proof.shutdown + 1] = exited
  client = nil
  check_phase('server shut down cleanly', exited.clean, exited)
end
local function snapshot_child_processes()
  if vim.fn.has('win32') ~= 1 or not proof.hostPids[#proof.hostPids] then return end
  local host_pid = proof.hostPids[#proof.hostPids]
  local script = ('Get-CimInstance Win32_Process -Filter \'ParentProcessId = %d\' | Select-Object ProcessId,Name | ConvertTo-Json -Compress'):format(host_pid)
  local observed = host_system({ 'pwsh', '-NoProfile', '-Command', script }, { text = true, timeout = 5000 }):wait(5500)
  proof.childProcesses[#proof.childProcesses + 1] = { hostPid = host_pid, exit = observed.code, raw = observed.stdout, stderr = observed.stderr }
end
local function request(label, command, payload, bufnr)
  flush()
  local complete, value, problem = false, nil, nil
  local deadline = (command == 'mcdev.reindex' or command == 'mcdev.reloadProjectContext') and 120000 or 25000
  local request_record = { case = label, command = command, payload = vim.deepcopy(payload), attempts = 1, deadlineMs = deadline }
  proof.requests[#proof.requests + 1] = request_record
  local cancel = protocol.request(command, payload, function(result, err)
    complete, value, problem = true, result, err
    request_record.result, request_record.error = saved(result), saved(err)
  end, bufnr)
  local done = vim.wait(deadline, function() return complete end, 10)
  request_record.completed = done
  if not done then
    request_record.timedOut = true
    local cancelled, cancel_error = pcall(function() cancel() end)
    request_record.cancelRequested = cancelled
    if not cancelled then request_record.cancelError = tostring(cancel_error) end
  end
  save()
  check_phase(label .. ' received one response', done, request_record)
  return value, problem
end
local function domain_payload(bufnr, position)
  return { context = protocol.context(bufnr, position) }
end
local function exact(label, actual, expected)
  check(label, vim.deep_equal(actual, expected), { actual = actual, expected = expected })
end
local function no_measurements(response)
  local copy = vim.deepcopy(response)
  local debug = copy.result and copy.result.debug
  if debug then
    for _, key in ipairs({ 'totalMs', 'payloadBytes', 'bufferTextBytes', 'loadSessionMs', 'dtoConvertMs' }) do debug[key] = nil end
  end
  return copy
end
local function expected_completion(document_uri, version)
  return {
    protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'completion' },
    result = {
      items = {{
        label = 'method = "…"', detail = '@Inject attribute',
        documentation = 'Insert the method attribute and place the cursor in its value.',
        filterText = 'method', insertText = 'method = "${1}"$0', kind = 'keyword',
        sortKey = '0000_00_method',
        edit = { range = { start = { line = 6, character = 21 }, ['end'] = { line = 6, character = 25 } }, newText = 'method = "${1}"$0' },
        additionalEdits = {}, metadata = { name = 'method', source = 'mixin.attribute' }, insertTextFormat = 'snippet',
      }},
      isIncomplete = false, warnings = {},
      debug = {
        bindingFailedCount = 0, bindingResolvedCount = 0, bufferTextFallbackUsed = true,
        candidateCountAfterFilter = 1, candidateCountBeforeFilter = 1, command = 'mcdev.completion',
        completionContextKind = 'ATTRIBUTE', documentUri = document_uri, documentVersion = version,
        fallbackAnnotationContextReason = 'semantic completion context was unavailable',
        fallbackAnnotationContextUsed = true, inFlightDedupHit = false, languageId = 'java',
        negativeCacheHit = false, owner = 'Target', parseConfidence = 'MEDIUM',
        parseSource = 'HAND_WRITTEN_FALLBACK', projectSessionCacheHit = false,
        projectSessionVersion = 0, semanticContextFound = false, semanticMemberCount = 0,
        semanticTargetCount = 1, usedCompilationUnit = false, usedJavaProject = false,
        warnings = { 'FALLBACK_ANNOTATION_CONTEXT_USED: semantic completion context was unavailable' },
      },
    },
  }
end
local function expected_diagnostic()
  local line = vim.api.nvim_buf_get_lines(buffers.patch, 0, 1, false)[1]
  local start = assert(line:find('class Patch', 1, true)) - 1
  local config_uri = vim.uri_from_fname(project .. '/src/main/resources/fixture.mixins.json')
  return {
    protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'diagnostics' },
    result = { diagnostics = {{
      code = 'MIXIN_CLASS_NOT_LISTED_IN_CONFIG', severity = 'warning',
      message = "Mixin class 'Patch' is not listed in mixin config",
      range = { start = { line = 0, character = start }, ['end'] = { line = 0, character = start + 11 } },
      metadata = { configPath = config_uri, mixinClass = 'Patch' },
    }} },
  }
end
local function expected_hover()
  local line = vim.api.nvim_buf_get_lines(buffers.patch, 0, 1, false)[1]
  local start = assert(line:find('Target.class', 1, true)) - 1
  local ticks = string.rep(string.char(96), 3)
  return {
    protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'hover' },
    result = {
      contents = { ticks .. 'mcdev\nclass fixture.Target\nowner: fixture/Target\nnamespace: named\n' .. ticks },
      range = { start = { line = 0, character = start }, ['end'] = { line = 0, character = start + #'Target.class' } },
    },
  }
end
local function expected_definition()
  local line = vim.api.nvim_buf_get_lines(buffers.expression, 8, 9, false)[1]
  local start = assert(line:find('drawCall', 1, true)) - 1
  return {
    protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'definition' },
    result = { locations = {{
      documentUri = uri(buffers.expression),
      range = { start = { line = 8, character = start }, ['end'] = { line = 8, character = start + #'drawCall' } },
      metadata = { kind = 'class', namespace = 'NAMED', owner = '', name = 'drawCall' },
      resolution = 'SOURCE',
    }} },
  }
end
local function expected_incomplete()
  return {
    protocolVersion = 1, serverVersion = '0.1.0', capabilities = {},
    error = {
      code = 'INCOMPLETE_PROJECT_CONTEXT',
      message = 'complete project source-set and resource inventory is unavailable', details = {},
    },
  }
end
local function absolute_cursor(bufnr, row, token)
  local line = vim.api.nvim_buf_get_lines(bufnr, row - 1, row, false)[1]
  return { row, assert(line:find(token, 1, true)) - 1 + #token }
end
local function completion_request(label)
  local pos = absolute_cursor(buffers.owner, 7, 'meth')
  local payload = protocol.build_completion_payload(buffers.owner, pos)
  check('UTF-16 cursor includes inline emoji', payload.context.position.character == 25, payload.context.position)
  check('unsaved LSP version is explicit', type(payload.context.documentVersion) == 'number', payload.context)
  local response, err = request(label, 'mcdev.completion', payload, buffers.owner)
  check(label .. ' transport succeeded', err == nil, err)
  exact(label .. ' complete domain envelope', no_measurements(response),
    expected_completion(uri(buffers.owner), payload.context.documentVersion))
  return response, payload
end
local function raw_outer(command, bufnr, payload, version)
  return { command = 'ktlsp.providerCommand', arguments = {{
    provider = provider, command = command, context = {
      workspaceRoot = project, documentUri = uri(bufnr), bufferText = full_text(bufnr),
      documentVersion = version,
    }, arguments = { payload },
  }} }
end

local completion
local ok, failure = xpcall(function()
  vim.system = function(cmd, opts, callback)
    if type(cmd) ~= 'table' or cmd[1] ~= binary then return host_system(cmd, opts, callback) end
    local number = #proof.hostPids + 1
    local function capture(name)
      local file = assert(io.open(artifacts .. '/' .. name .. '-' .. number .. '.raw', 'wb'))
      return function(data)
        if data then file:write(data); file:flush() end
      end
    end
    local incoming, outgoing, errors = capture('wire-host-to-editor'), capture('wire-editor-to-host'), capture('host-stderr')
    local stdout, stderr = opts.stdout, opts.stderr
    opts.stdout = function(err, data) incoming(data); if stdout then stdout(err, data) end end
    opts.stderr = function(err, data) errors(data); if stderr then stderr(err, data) end end
    local process = host_system(cmd, opts, callback)
    proof.hostPids[#proof.hostPids + 1] = process.pid
    return {
      pid = process.pid,
      write = function(_, data) outgoing(data); return process:write(data) end,
      is_closing = function() return process:is_closing() end,
      kill = function(_, signal) return process:kill(signal) end,
    }
  end
  vim.lsp.log.debug = function(label, message, ...)
    if label == 'rpc.receive' and cancel_id ~= nil and type(message) == 'table' and message.id == cancel_id then
      cancel_wire = vim.deepcopy(message)
    end
    return host_debug(label, message, ...)
  end
  for key, path in pairs(paths) do
    local bufnr = vim.fn.bufadd(path)
    vim.fn.bufload(bufnr)
    vim.bo[bufnr].filetype = key == 'aw' and 'accesswidener' or key == 'kotlin' and 'kotlin' or 'java'
    if key == 'kotlin' then
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { 'package fixture', 'class NoActions' })
    end
    buffers[key] = bufnr
  end
  run_case('original E2E startup support', nil, function()
    local startup = vim.fs.normalize(repo .. '/mcdev-nvim/tests/e2e/minimal_init.lua')
    local sourced = false
    for _, script in ipairs(vim.fn.getscriptinfo()) do
      if vim.fs.normalize(script.name) == startup then sourced = true; break end
    end
    local runtime_root = vim.fs.normalize(repo .. '/mcdev-nvim')
    local protocol_source = vim.fs.normalize(debug.getinfo(protocol.context, 'S').source:sub(2))
    proof.startup = {
      path = startup, sourced = sourced, swapfileDisabled = not vim.o.swapfile,
      runtimeRoot = vim.fs.normalize(vim.api.nvim_list_runtime_paths()[1]), protocolSource = protocol_source,
    }
    check('preserved original startup was actually sourced', sourced, proof.startup)
    check('original startup disables swap files for fixture buffers',
      not vim.o.swapfile and not vim.bo[buffers.owner].swapfile, proof.startup)
    exact('original startup prepends the MC runtime', proof.startup.runtimeRoot, runtime_root)
    exact('MC protocol loads from the original startup runtime', protocol_source,
      runtime_root .. '/lua/mcdev/protocol.lua')
    proof.startup.verified = true
    return proof.startup
  end)
  vim.api.nvim_set_current_buf(buffers.owner)
  local initial = acceptance:mark()
  exited = start_server()
  run_case('initial provider accepts the current owner', nil, function()
    return await_accepted(buffers.owner, initial)
  end)
  snapshot_child_processes()

  local before = vim.lsp.util.buf_versions[buffers.owner]
  local floor = acceptance:mark()
  vim.api.nvim_buf_set_lines(buffers.owner, 3, 4, false, { '// 未保存' })
  vim.api.nvim_buf_set_lines(buffers.owner, 6, 7, false, { '    /* 😀 */ @Inject(meth)' })
  flush()
  local after = vim.lsp.util.buf_versions[buffers.owner]
  run_case('unsaved Unicode source and current LSP version', nil, function()
    check('buffer is unsaved', vim.bo[buffers.owner].modified, nil)
    check('disk still contains saved method attribute', vim.fn.readfile(paths.owner)[7] == '    @Inject(method = "tick")', nil)
    check('LSP version advanced', type(before) == 'number' and type(after) == 'number' and after > before, { before = before, after = after })
    return await_accepted(buffers.owner, floor)
  end)
  local completed, result = run_case('full Unicode completion DTO', 'mcdev.completion', function()
    return completion_request('current Unicode completion')
  end)
  if completed then completion = result end
  run_case('apply real completion snippet to current buffer', nil, function()
    check('completion exists', completion ~= nil, completion)
    local item = completion.result.items[1]
    local edit = item.edit
    local line = vim.api.nvim_buf_get_lines(buffers.owner, 6, 7, false)[1]
    local byte_start = vim.str_byteindex(line, 'utf-16', edit.range.start.character)
    local byte_end = vim.str_byteindex(line, 'utf-16', edit.range['end'].character)
    vim.api.nvim_set_current_buf(buffers.owner)
    vim.api.nvim_buf_set_text(buffers.owner, edit.range.start.line, byte_start,
      edit.range['end'].line, byte_end, { '' })
    vim.api.nvim_win_set_cursor(0, { edit.range.start.line + 1, byte_start })
    vim.snippet.expand(edit.newText)
    check('snippet expansion applied exact buffer text',
      vim.api.nvim_buf_get_lines(buffers.owner, 6, 7, false)[1] == '    /* 😀 */ @Inject(method = "")',
      vim.api.nvim_buf_get_lines(buffers.owner, 6, 7, false)[1])
    proof.appliedBuffer = full_text(buffers.owner)
    return proof.appliedBuffer
  end)
  vim.api.nvim_buf_set_lines(buffers.owner, 6, 7, false, { '    /* 😀 */ @Inject(meth)' })
  flush()
  run_case('stale document version is rejected', nil, function()
    local payload = protocol.build_completion_payload(buffers.owner, absolute_cursor(buffers.owner, 7, 'meth'))
    local version = assert(vim.lsp.util.buf_versions[buffers.owner])
    local params = raw_outer('mcdev.completion', buffers.owner, payload, version - 1)
    local response, err = client:request_sync('workspace/executeCommand', params, 10000, buffers.owner)
    proof.requests[#proof.requests + 1] = { case = 'stale', command = 'mcdev.completion', params = params, response = saved(response), error = saved(err), attempts = 1 }
    check_phase('stale-version request completed', response ~= nil, { response = response, error = err })
    check('host rejects stale version', response and response.err and response.err.code == -32801, response or err)
    return response
  end)
  for _, command in ipairs(incomplete_ids) do
    run_case('typed incomplete project result: ' .. command, nil, function()
      local pos = command == 'mcdev.references' and absolute_cursor(buffers.patch, 1, 'Target') or { 1, 0 }
      local value, err = request('incomplete ' .. command, command, domain_payload(buffers.patch, pos), buffers.patch)
      check('incomplete command transport succeeded', err == nil, err)
      exact('complete typed incomplete envelope ' .. command, value, expected_incomplete())
      if command == 'mcdev.reindex' or command == 'mcdev.reloadProjectContext' then
        local outer = proof.frames[#proof.frames].params.arguments[1]
        check(command .. ' requested a real host refresh', outer.refreshProject == true, outer)
      end
      return value
    end)
  end
  run_case('full AW header empty references DTO', 'mcdev.references', function()
    exact('AW header has no reference target', full_text(buffers.aw), 'accessWidener v2 named\n')
    check('AW buffer is attached to the real host', vim.lsp.buf_is_attached(buffers.aw, client.id), nil)
    local payload = domain_payload(buffers.aw, { 1, 0 })
    exact('AW request uses the actual buffer language', payload.context.languageId, 'accesswidener')
    check('AW request carries the current LSP document version',
      type(payload.context.documentVersion) == 'number'
        and payload.context.documentVersion == vim.lsp.util.buf_versions[buffers.aw], payload.context)
    local value, err = request('AW header empty references', 'mcdev.references', payload, buffers.aw)
    check('empty references transport succeeded', err == nil, err)
    exact('complete empty references envelope', value, {
      protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'references' }, result = { locations = {} },
    })
    return value
  end)
  run_case('full Kotlin empty code action DTO', 'mcdev.codeAction', function()
    local value, err = request('Kotlin empty code action', 'mcdev.codeAction', domain_payload(buffers.kotlin, { 1, 0 }), buffers.kotlin)
    check('Kotlin code action transport succeeded', err == nil, err)
    exact('complete Kotlin empty code action envelope', value, {
      protocolVersion = 1, serverVersion = '0.1.0', capabilities = { 'codeAction' }, result = { actions = {} },
    })
    return value
  end)
  for _, command in ipairs({ 'mcdev.diagnostics', 'mcdev.context' }) do
    run_case('full config diagnostic DTO ' .. command, command, function()
      local value, err = request(command, command, domain_payload(buffers.patch, { 1, 0 }), buffers.patch)
      check('diagnostic transport succeeded', err == nil, err)
      exact('complete diagnostic envelope', value, expected_diagnostic())
      return value
    end)
  end
  run_case('full Mixin hover DTO', 'mcdev.hover', function()
    local pos = absolute_cursor(buffers.patch, 1, 'Target')
    local value, err = request('hover', 'mcdev.hover', domain_payload(buffers.patch, pos), buffers.patch)
    check('hover transport succeeded', err == nil, err)
    exact('complete hover envelope', value, expected_hover())
    return value
  end)
  run_case('full MixinExtras definition DTO', 'mcdev.definition', function()
    local pos = absolute_cursor(buffers.expression, 11, 'drawCall')
    pos[2] = pos[2] - #'drawCall' + 2
    local line = vim.api.nvim_buf_get_lines(buffers.expression, 10, 11, false)[1]
    local token_start = assert(line:find('drawCall', 1, true)) - 1
    local payload = domain_payload(buffers.expression, pos)
    proof.definitionCursorOracle = { lineText = line, tokenStartByte = token_start, cursorByte = pos[2], position = payload.context.position }
    check('definition cursor is inside identifier at token start plus two', pos[2] == token_start + 2 and line:sub(pos[2] + 1, pos[2] + 1) == 'a', proof.definitionCursorOracle)
    check('definition cursor has expected UTF-16 position', payload.context.position.line == 10 and payload.context.position.character == 19, proof.definitionCursorOracle)
    local value, err = request('definition', 'mcdev.definition', payload, buffers.expression)
    check('definition transport succeeded', err == nil, err)
    exact('complete definition envelope', value, expected_definition())
    return value
  end)
  run_case('real MixinExtras definition navigation', 'mcdev.definition', function()
    local previous = {
      window = vim.api.nvim_get_current_win(), buffer = vim.api.nvim_get_current_buf(),
      cursor = vim.api.nvim_win_get_cursor(0), modified = vim.bo[buffers.expression].modified,
    }
    local original = vim.api.nvim_buf_get_lines(buffers.expression, 8, 9, false)[1]
    local passed, result = xpcall(function()
      local destination = '/* 日😀 */ ' .. original
      local target_byte = assert(destination:find('drawCall', 1, true)) - 1
      local target_utf16 = vim.str_utfindex(destination, 'utf-16', target_byte, false)
      check('navigation fixture requires UTF-16 to byte conversion', target_byte > target_utf16,
        { byte = target_byte, utf16 = target_utf16 })
      local floor = acceptance:mark()
      vim.api.nvim_buf_set_lines(buffers.expression, 8, 9, false, { destination })
      vim.api.nvim_set_current_buf(buffers.expression)
      local source = absolute_cursor(buffers.expression, 11, 'drawCall')
      source[2] = source[2] - #'drawCall' + 2
      vim.api.nvim_win_set_cursor(0, source)
      flush()
      await_accepted(buffers.expression, floor)
      local complete, locations, navigation_error, raw_locations = false, nil, nil, nil
      local frame_count = #proof.frames
      require('mcdev.navigation').definition(buffers.expression, nil, function(found, err, raw)
        locations, navigation_error, raw_locations, complete = found, err, raw, true
      end)
      local navigated = vim.wait(25000, function() return complete end, 10)
      if not navigated then
        local pending = proof.frames[frame_count + 1]
        if pending then
          pending.timedOut = true
          local cancelled, result = pcall(function()
            return pending.requestId ~= nil and client:cancel_request(pending.requestId)
          end)
          pending.cancelRequested = cancelled and result == true
          if not cancelled then pending.cancelError = tostring(result) end
        end
      end
      check_phase('public navigation callback completed', navigated, proof.frames[frame_count + 1])
      check('public navigation succeeded', navigation_error == nil, navigation_error)
      exact('navigation sent one real provider request', #proof.frames, frame_count + 1)
      local frame = proof.frames[frame_count + 1]
      local outer = frame.params.arguments[1]
      exact('navigation uses attached ktlsp provider command',
        { frame.params.command, outer.provider, outer.command },
        { 'ktlsp.providerCommand', provider, 'mcdev.definition' })
      exact('navigation sends the current expression UTF-16 position',
        outer.arguments[1].context.position, { line = 10, character = 19 })
      local expected_range = {
        start = { line = 8, character = target_utf16 },
        ['end'] = { line = 8, character = target_utf16 + #'drawCall' },
      }
      exact('navigation retains the exact selectable source location', locations, {{
        uri = uri(buffers.expression), range = expected_range, resolution = 'SOURCE',
      }})
      exact('navigation retains the original source range', raw_locations[1].range, expected_range)
      local shown = vim.lsp.util.show_document(locations[1], 'utf-16', { focus = true })
      check('real show_document accepted the source location', shown == true, shown)
      check('real show_document moved the editor to the selected Definition', vim.wait(10000, function()
        return vim.api.nvim_get_current_win() == previous.window
          and vim.api.nvim_get_current_buf() == buffers.expression
          and vim.uri_from_bufnr(vim.api.nvim_get_current_buf()) == locations[1].uri
          and vim.deep_equal(vim.api.nvim_win_get_cursor(0), { 9, target_byte })
      end, 20), { window = vim.api.nvim_get_current_win(), buffer = vim.api.nvim_get_current_buf(),
        cursor = vim.api.nvim_win_get_cursor(0), expected = { 9, target_byte } })
      proof.navigation = {
        sourceCursor = source, selectedLocation = vim.deepcopy(locations[1]),
        window = vim.api.nvim_get_current_win(), buffer = vim.api.nvim_get_current_buf(),
        documentUri = vim.uri_from_bufnr(vim.api.nvim_get_current_buf()), cursor = vim.api.nvim_win_get_cursor(0),
        expectedCursor = { 9, target_byte },
      }
      return proof.navigation
    end, debug.traceback)
    vim.api.nvim_buf_set_lines(buffers.expression, 8, 9, false, { original })
    vim.bo[buffers.expression].modified = previous.modified
    vim.api.nvim_set_current_win(previous.window)
    vim.api.nvim_win_set_buf(previous.window, previous.buffer)
    vim.api.nvim_win_set_cursor(previous.window, previous.cursor)
    flush()
    if not passed then error(result) end
    return result
  end)
  run_case('cancel command through real host', nil, function()
    flush()
    local params = raw_outer('mcdev.diagnostics', buffers.patch,
      domain_payload(buffers.patch, { 1, 0 }), vim.lsp.util.buf_versions[buffers.patch])
    local received
    local sent, id = client:request('workspace/executeCommand', params, function(err, value)
      received = { err = saved(err), result = saved(value) }
    end, buffers.patch)
    check('cancel request was sent', sent and id ~= nil, id)
    cancel_id, cancel_wire = id, nil
    check('cancel notification was sent', client:cancel_request(id), id)
    local measured = vim.wait(10000, function() return cancel_wire ~= nil or received ~= nil end, 10)
    local wire = cancel_wire or received
    proof.requests[#proof.requests + 1] = { case = 'cancel', command = 'mcdev.diagnostics', params = params, response = saved(wire), attempts = 1 }
    cancel_id = nil
    check_phase('host returned cancellation', measured and wire and wire.error and wire.error.code == -32800, wire)
    return wire
  end)
  run_case('disabled provider rejects a command', nil, function()
    check('disable notification sent', client:notify('workspace/didChangeConfiguration',
      { settings = { analysisProviders = { [provider] = provider_settings(false) } } }), nil)
    local payload = protocol.build_completion_payload(buffers.owner, absolute_cursor(buffers.owner, 7, 'meth'))
    local value, err = request('disabled', 'mcdev.completion', payload, buffers.owner)
    check('disabled provider is unavailable', value == nil and type(err) == 'table' and err.code == -32002, { value, err })
    return err
  end)
  local reenabled_floor = acceptance:mark()
  run_case('reenabled provider uses a fresh instance', nil, function()
    check('enable notification sent', client:notify('workspace/didChangeConfiguration',
      { settings = { analysisProviders = { [provider] = provider_settings(true) } } }), nil)
    local found = await_accepted(buffers.owner, reenabled_floor, { fresh_instance = true })
    local response = completion_request('reenabled completion')
    return { accepted = found, response = response }
  end)
  run_case('restart host and companion', nil, function()
    local previous_host = proof.hostPids[#proof.hostPids]
    stop_server(exited)
    acceptance:close()
    vim.env.KTLSP_TRACE = artifacts .. '/host-restart.trace.jsonl'
    acceptance = acceptance_module.start(vim.env.KTLSP_TRACE, 'mcdev.mixin')
    local floor_after_stop = acceptance:mark()
    exited = start_server()
    check('restart creates a new host process',
      proof.hostPids[#proof.hostPids] ~= previous_host, proof.hostPids)
    local found = await_accepted(buffers.owner, floor_after_stop, { fresh_instance = true })
    local response = completion_request('restarted completion')
    return { accepted = found, response = response }
  end)
end, debug.traceback)
if client then
  local clean, err = pcall(stop_server, exited or { exited = false })
  if not clean then failure = (failure and tostring(failure) .. '\n' or '') .. tostring(err); ok = false end
end
acceptance:close()
vim.system, vim.lsp.log.debug = host_system, host_debug
proof.error = ok and vim.NIL or tostring(failure)
local required = {
  ['full Unicode completion DTO'] = true, ['full config diagnostic DTO mcdev.diagnostics'] = true,
  ['full config diagnostic DTO mcdev.context'] = true, ['full Mixin hover DTO'] = true,
  ['full MixinExtras definition DTO'] = true, ['apply real completion snippet to current buffer'] = true,
  ['stale document version is rejected'] = true, ['cancel command through real host'] = true,
  ['disabled provider rejects a command'] = true, ['reenabled provider uses a fresh instance'] = true,
  ['restart host and companion'] = true, ['unsaved Unicode source and current LSP version'] = true,
  ['full AW header empty references DTO'] = true, ['full Kotlin empty code action DTO'] = true,
  ['real MixinExtras definition navigation'] = true,
  ['original E2E startup support'] = true,
}
for _, command in ipairs(incomplete_ids) do
  required['typed incomplete project result: ' .. command] = true
end
for _, row in ipairs(proof.matrix) do
  if row.status == 'passed' then required[row.case] = nil end
end
proof.scopePassed = ok and not proof.indexingIncomplete and next(required) == nil
proof.missingRequired = required
save()
if not proof.scopePassed then
  io.stderr:write('Command scope incomplete: ' .. vim.inspect({ error = failure, missing = required }) .. '\n')
  vim.cmd('cquit 1')
end
print('command editor smoke: PASS (' .. proof.checks .. ' checks)')
vim.cmd('qa!')
