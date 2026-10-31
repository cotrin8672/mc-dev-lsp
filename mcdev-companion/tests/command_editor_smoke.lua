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
local proof = {
  checks = 0, assertions = {}, matrix = {}, requests = {}, frames = {},
  indexing = {}, shutdown = {}, hostPids = {}, childProcesses = {},
  editorPid = vim.uv.os_getpid(), gradleBuildSucceeded = vim.env.MCDEV_COMMAND_GRADLE_BUILD_OK == '1',
  completeReplacementVerified = false, sdkSourceFixture = 'reduced Object.java',
}
local client, buffers, exited = nil, {}, nil
local host_system, host_debug = vim.system, vim.lsp.log.debug
local cancel_id, cancel_wire
local positive
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
  if positive and positive.enabled then request_record.traceStart = positive.trace_mark() end
  proof.requests[#proof.requests + 1] = request_record
  local cancel = protocol.request(command, payload, function(result, err)
    complete, value, problem = true, result, err
    request_record.result, request_record.error = saved(result), saved(err)
  end, bufnr)
  local done = vim.wait(deadline, function() return complete end, 10)
  request_record.completed = done
  if positive and positive.enabled then request_record.traceEnd = positive.trace_mark() end
  if not done then
    request_record.timedOut = true
    local cancelled, cancel_error = pcall(function() cancel() end)
    request_record.cancelRequested = cancelled
    if not cancelled then request_record.cancelError = tostring(cancel_error) end
  end
  save()
  check_phase(label .. ' received one response', done, request_record)
  return value, problem, request_record
end
local function domain_payload(bufnr, position)
  return { context = protocol.context(bufnr, position) }
end
local function exact(label, actual, expected)
  check(label, vim.deep_equal(actual, expected), { actual = actual, expected = expected })
end
-- Fixed DTOs come only from the repeated old Kotlin artifact. Dynamic model lines
-- follow the frozen original InfoService, with independently checked fixture paths.
positive = { trace = { path = nil, offset = 0, partial = '', events = {} } }
function positive.shape(label, value, required, optional)
  check(label .. ' is an object', type(value) == 'table' and value ~= vim.NIL, value)
  local allowed = {}
  for _, key in ipairs(required) do
    allowed[key] = true
    check(label .. '.' .. key .. ' is present', value[key] ~= nil and value[key] ~= vim.NIL, value)
  end
  for _, key in ipairs(optional or {}) do allowed[key] = true end
  for key in pairs(value) do check(label .. ' has no unexpected field ' .. tostring(key), allowed[key] == true, value) end
end
function positive.path(value)
  assert(type(value) == 'string', 'path must be a string')
  if value:match('^file:') then value = vim.uri_to_fname(value) end
  value = vim.fs.normalize(value):gsub('^//%?/', ''):gsub('/+$', '')
  return vim.fn.has('win32') == 1 and value:lower() or value
end
function positive.read(path)
  local file = assert(io.open(path, 'rb'))
  local value = assert(file:read('*a')); file:close(); return value
end
function positive.path_set(label, values, expected)
  check(label .. ' is a list', type(values) == 'table' and vim.islist(values), values)
  local paths, seen = {}, {}
  for _, path in ipairs(values) do
    check(label .. ' contains absolute paths', type(path) == 'string' and (path:match('^%a:[/\\]') or path:match('^/')) ~= nil, path)
    local key = positive.path(path)
    check(label .. ' has no duplicate path', not seen[key], path)
    seen[key] = true; paths[#paths + 1] = key
  end
  table.sort(paths)
  if expected then
    local wanted = {}; for _, path in ipairs(expected) do wanted[#wanted + 1] = positive.path(path) end
    table.sort(wanted); exact(label .. ' is the exact expected set', paths, wanted)
  end
  return seen
end
function positive.envelope(label, value, capabilities)
  positive.shape(label, value, { 'protocolVersion', 'serverVersion', 'capabilities', 'result' })
  exact(label .. ' protocol', value.protocolVersion, 1)
  exact(label .. ' server', value.serverVersion, '0.1.0')
  local caps = vim.deepcopy(capabilities); table.sort(caps)
  exact(label .. ' capability set', value.capabilities, caps)
  return value.result
end
function positive.load()
  local path = repo .. '/mcdev-companion/tests/fixtures/host_commands/original-success.json'
  positive.oracle = vim.json.decode(positive.read(path))
  exact('old oracle is independent of Rust output', positive.oracle.provenance.rustOutputUsed, false)
  exact('old oracle retains its build provenance limit', positive.oracle.provenance.compiledFromFrozenBaselineProven, false)
  exact('old oracle does not claim full JDT routing', positive.oracle.provenance.fullJdtRouteProven, false)
  local build = assert(vim.env.MCDEV_COMMAND_BUILD_INPUT, 'pinned build identity evidence is required')
  positive.build = vim.json.decode(positive.read(build))
  positive.shape('build input', positive.build, { 'artifactPath', 'artifactSha256', 'commit', 'buildTime', 'version', 'runStartedAtUnixMs', 'receiptSha256' })
  exact('build input names the pinned real companion', positive.path(positive.build.artifactPath), positive.path(companion))
  for _, key in ipairs({ 'commit', 'buildTime', 'version' }) do
    check('build input records ' .. key, type(positive.build[key]) == 'string' and #positive.build[key] > 0, positive.build)
  end
  proof.positiveOracle = { path = path, sha256 = vim.fn.sha256(positive.read(path)), provenance = vim.deepcopy(positive.oracle.provenance) }
  proof.buildInput = vim.deepcopy(positive.build)
  positive.enabled = true
end
function positive.oracle_result(name)
  local result = vim.deepcopy(positive.oracle[name])
  local root = vim.uri_from_fname(project):gsub('/+$', '')
  local function expand(value)
    assert(value:sub(1, #'${WORKSPACE_URI}') == '${WORKSPACE_URI}')
    return root .. value:sub(#'${WORKSPACE_URI}' + 1)
  end
  if name == 'references' then
    for _, location in ipairs(result.locations) do location.documentUri = expand(location.documentUri) end
  else
    for _, action in ipairs(result.actions) do
      action.metadata.configPath = expand(action.metadata.configPath)
      for _, edit in ipairs(action.edits) do edit.documentUri = expand(edit.documentUri) end
    end
  end
  return result
end
function positive.references(value)
  positive.envelope('positive references', value, { 'references' })
  local function locations(result)
    positive.shape('references result', result, { 'locations' })
    local output, seen = {}, {}
    for _, item in ipairs(result.locations) do
      positive.shape('reference location', item, { 'documentUri', 'range', 'metadata', 'resolution' })
      check('reference URI is a file URI', item.documentUri:match('^file:') ~= nil, item)
      local copy = vim.deepcopy(item); copy.documentUri = positive.path(copy.documentUri)
      local key = copy.documentUri .. ':' .. vim.json.encode(copy.range)
      check('positive reference location has no duplicate', not seen[key], item); seen[key] = true
      output[#output + 1] = copy
    end
    table.sort(output, function(a, b) return a.documentUri < b.documentUri end)
    return output
  end
  exact('all original reference URIs/ranges/metadata/resolutions', locations(value.result), locations(positive.oracle_result('references')))
end
function positive.lines(model, version)
  local mappings, cp = model.mappings, model.classpath
  local available = vim.deepcopy(mappings.availableNamespaces)
  local rank = { named = 1, intermediary = 2, official = 3, srg = 4, mcp = 5 }
  table.sort(available, function(a, b) return rank[a] < rank[b] end)
  local function count(name, n) return name .. (n == 0 and ': none' or n == 1 and ': 1 file' or ': ' .. n .. ' files') end
  return {
    'Project: ' .. model.projectId, 'Root: ' .. model.root, 'Platform: Unknown',
    #available == 0 and 'Mappings: none' or ('Mappings: ' .. mappings.sourceNamespace .. ' <-> ' .. mappings.runtimeNamespace .. (#available >= 2 and ' loaded' or ' partial')),
    #available == 0 and 'Mapping source: none (no supported mapping files discovered)' or 'Mapping source: provided',
    'Source namespace: ' .. mappings.sourceNamespace, 'Runtime namespace: ' .. mappings.runtimeNamespace,
    'Minecraft jar: none', count('Mixin config', #model.mixinConfigs),
    'Mixin config search: ' .. table.concat(model.mixinConfigs, ', '),
    count('Access Widener', #model.accessWideners), count('Access Transformer', #model.accessTransformers),
    'Classpath entries: ' .. cp.entryCount,
    ('Classpath source: project outputs=%d, dependencies=%d, minecraft=%d'):format(#cp.projectOutputs, #cp.dependencyJars, #cp.minecraftJars),
    'Source sets: ' .. #model.sourceSets, 'Class index: ready', 'Bytecode index: ready',
    #available == 0 and 'Mapping namespaces: none (searched project mapping files and known Gradle mapping caches)' or 'Mapping namespaces: ' .. table.concat(available, ', '),
    'Protocol: 1', 'Extension: ' .. version,
  }
end
function positive.dump(value, configs)
  local data = positive.envelope('positive dump', value, { 'dumpContext', 'info' })
  positive.shape('dump result', data, { 'lines', 'projectId', 'root', 'platform', 'mappings', 'classpath', 'sourceSets', 'mixinConfigs', 'accessWideners', 'accessTransformers', 'minecraftJars', 'indexState' })
  check('project id is a nonempty opaque owning-model id', type(data.projectId) == 'string' and #data.projectId > 0, data.projectId)
  exact('dump belongs to the isolated root', positive.path(data.root), positive.path(project))
  exact('Java-only fixture platform', data.platform, 'unknown')
  exact('accepted index is ready', data.indexState, 'ready')
  positive.path_set('selected mixin configs', data.mixinConfigs, configs)
  positive.path_set('selected AW files', data.accessWideners, { paths.aw })
  positive.path_set('selected AT files', data.accessTransformers, {})
  positive.path_set('selected Minecraft jars', data.minecraftJars, {})
  local mappings = data.mappings
  positive.shape('mapping DTO', mappings, { 'sourceNamespace', 'runtimeNamespace', 'availableNamespaces' }, { 'awNamespace', 'atNamespace' })
  local namespaces = { named = true, intermediary = true, official = true, srg = true, mcp = true }
  for _, key in ipairs({ 'sourceNamespace', 'runtimeNamespace', 'awNamespace', 'atNamespace' }) do
    check('known optional/required mapping namespace ' .. key, mappings[key] == nil or namespaces[mappings[key]] == true, mappings)
  end
  local available, seen = {}, {}
  for _, name in ipairs(mappings.availableNamespaces) do
    check('available namespaces are known and unique', namespaces[name] and not seen[name], mappings)
    seen[name] = true; available[#available + 1] = name
  end
  table.sort(available); exact('available namespace DTO is canonical', mappings.availableNamespaces, available)
  local cp = data.classpath
  positive.shape('classpath DTO', cp, { 'entryCount', 'projectOutputs', 'dependencyJars', 'minecraftJars', 'generatedOutputs', 'capturedAt' })
  local total, outputs = 0, {}
  for _, key in ipairs({ 'projectOutputs', 'dependencyJars', 'minecraftJars', 'generatedOutputs' }) do
    local set = positive.path_set('classpath ' .. key, cp[key])
    total = total + #cp[key]
    if key == 'projectOutputs' or key == 'generatedOutputs' then for path in pairs(set) do outputs[path] = true end end
    if key == 'dependencyJars' then for _, path in ipairs(cp[key]) do
      local stat = vim.uv.fs_stat(path); check('selected dependency is a real file', stat and stat.type == 'file', path)
    end end
  end
  exact('classpath count follows the actual reported inventory', cp.entryCount, total)
  check('real project class output is selected', outputs[positive.path(project .. '/build/classes/java/main')] == true, cp)
  check('exact fixture Target.class exists in selected output', vim.uv.fs_stat(project .. '/build/classes/java/main/fixture/Target.class') ~= nil, cp)
  exact('Java-only fixture has no Minecraft classpath', cp.minecraftJars, {})
  check('capture time is real nonzero millisecond metadata', type(cp.capturedAt) == 'number' and cp.capturedAt % 1 == 0
    and cp.capturedAt >= positive.build.runStartedAtUnixMs and cp.capturedAt <= vim.fn.localtime() * 1000 + 999, cp)
  local main, names = nil, {}
  check('source sets are a nonempty list', type(data.sourceSets) == 'table' and vim.islist(data.sourceSets) and #data.sourceSets > 0, data.sourceSets)
  for _, set in ipairs(data.sourceSets) do
    positive.shape('source-set DTO', set, { 'name', 'sourceDirectories', 'resourceDirectories' }, { 'outputDirectory' })
    check('source-set name is nonempty and unique', type(set.name) == 'string' and #set.name > 0 and not names[set.name], set)
    names[set.name] = true
    local sources = positive.path_set('source directories', set.sourceDirectories)
    local resources = positive.path_set('resource directories', set.resourceDirectories)
    for _, list in ipairs({ set.sourceDirectories, set.resourceDirectories }) do for _, path in ipairs(list) do
      check('fixture source/resource root belongs to the isolated project', positive.path(path):sub(1, #positive.path(project) + 1) == positive.path(project) .. '/', path)
    end end
    if set.outputDirectory ~= nil then check('source-set output is in selected outputs', outputs[positive.path(set.outputDirectory)] == true, set) end
    if sources[positive.path(project .. '/src/main/java')] then
      check('main source set selects actual resources', resources[positive.path(project .. '/src/main/resources')] == true, set)
      check('main source set has selected output', set.outputDirectory ~= nil, set); main = set
    end
  end
  check('actual main source/resource set is present', main ~= nil, data.sourceSets)
  exact('all original project info lines in dump', data.lines, positive.lines(data, '0.1.0'))
  return data
end
function positive.info(value, model)
  local data = positive.envelope('positive info', value, { 'info', 'completion', 'diagnostics', 'hover', 'definition', 'references', 'codeAction', 'reindex', 'projectContext' })
  positive.shape('info result', data, { 'lines', 'buildCommit', 'buildTime', 'version', 'jarLocation', 'registeredCommands' })
  exact('Info commit matches pinned build receipt', data.buildCommit, positive.build.commit)
  exact('Info time matches pinned build receipt', data.buildTime, positive.build.buildTime)
  exact('Info version matches pinned build receipt', data.version, positive.build.version)
  exact('Info artifact is the real companion executable', positive.path(data.jarLocation), positive.path(positive.build.artifactPath))
  check('Info artifact exists', vim.uv.fs_stat(data.jarLocation) ~= nil, data.jarLocation)
  exact('exact original negotiated command registry', data.registeredCommands, positive.oracle.registeredCommands)
  local allowed, seen = {}, {}; for _, id in ipairs(command_ids) do allowed[id] = true end
  for _, id in ipairs(data.registeredCommands) do check('registry is implemented, negotiated and unique', allowed[id] and not seen[id], data); seen[id] = true end
  exact('registry has all 11 commands', #data.registeredCommands, 11)
  local lines = positive.lines(model, positive.build.version)
  vim.list_extend(lines, { 'Extension loaded: true', 'Extension build commit: ' .. positive.build.commit,
    'Extension build time: ' .. positive.build.buildTime, 'Extension jar location: ' .. data.jarLocation, 'Registered commands:' })
  for _, id in ipairs(positive.oracle.registeredCommands) do lines[#lines + 1] = '  - ' .. id end
  exact('every Info line matches original format and current project model', data.lines, lines)
  return data
end
function positive.trace_mark()
  local state, path = positive.trace, vim.env.KTLSP_TRACE
  if state.path ~= path then state.path, state.offset, state.partial, state.events = path, 0, '', {} end
  local file = assert(io.open(path, 'rb')); assert(file:seek('set', state.offset))
  local chunk = file:read('*a') or ''; file:close(); state.offset = state.offset + #chunk
  check('trace stays within the finite evidence budget', state.offset <= 128 * 1024 * 1024, state.offset)
  local data, consumed = state.partial .. chunk, 0
  for line, next_byte in data:gmatch('([^\n]*)\n()') do
    consumed = next_byte - 1
    if line:match('%S') then
      local event = vim.json.decode(line)
      if event.name == 'extension.ipc.command_state' or event.name == 'workspace.refresh_requested'
        or event.name == 'workspace.accepted_finish' or event.name == 'workspace.project_scan'
        or event.name == 'deps.index_workspace' or event.name == 'extension.ipc.read_facts' then
        state.events[#state.events + 1] = event
      end
    end
  end
  state.partial = data:sub(consumed + 1)
  return #state.events
end
function positive.ready(record, refresh)
  local events, selected, selected_index = positive.trace.events, nil, nil
  for index = record.traceStart + 1, record.traceEnd do
    local event, args = events[index], events[index].args
    if event.name == 'extension.ipc.command_state' and args.dispatch == true and args.stage == 'child-response'
      and args.provider == provider and args.command == record.command
      and positive.path(args.owner) == positive.path(record.payload.context.documentUri) then
      check('one dispatched command state matches this response', selected == nil, { selected, event }); selected, selected_index = event, index
    end
  end
  check('response has real host command-state evidence', selected ~= nil, record)
  local state = selected.args
  check('host command has an evaluation and worker identity', state.evaluation ~= nil and state.evaluation ~= vim.NIL
    and state.instance ~= nil and state.instance ~= vim.NIL, state)
  check('host command has an exact current document revision', type(state.expected_document_revision) == 'string'
    and #state.expected_document_revision > 0 and type(state.current_document_revision) == 'number'
    and state.current_document_revision % 1 == 0 and state.current_document_revision <= 9007199254740991
    and state.expected_document_revision == tostring(state.current_document_revision), state)
  exact('host command source hash matches current request', state.source_sha256, vim.fn.sha256(record.payload.context.bufferText))
  exact('host command version matches current request', state.expected_document_version, record.payload.context.documentVersion)
  exact('host command version is still current', state.current_document_version, record.payload.context.documentVersion)
  exact('host command evaluation is current', state.evaluation_current, true)
  check('host dispatch has no error', state.error == nil or state.error == vim.NIL, state)
  check('analysis generation is an exact observable token', type(state.analysis_generation) == 'number' and state.analysis_generation % 1 == 0 and state.analysis_generation <= 9007199254740991, state)
  local generation, accepted, indexed, scanned, requested = tostring(state.analysis_generation), nil, nil, nil, nil
  local accepted_index, indexed_index, scanned_index, requested_index
  local operations = {}
  for index = 1, selected_index do
    local event = events[index]
    local args = event.args
    if event.name == 'extension.ipc.read_facts' and tostring(args.evaluation) == tostring(state.evaluation)
      and args.instance == state.instance and args.command == true and args.ok == true then
      for _, op in ipairs(args.ops or {}) do operations[op] = true end
    end
    if args.generation ~= nil and tostring(args.generation) == generation
      and (args.root == nil or positive.path(args.root) == positive.path(project)) then
      if event.name == 'workspace.accepted_finish' then accepted, accepted_index = event, index end
      if event.name == 'deps.index_workspace' then indexed, indexed_index = event, index end
      if event.name == 'workspace.project_scan' then scanned, scanned_index = event, index end
      if event.name == 'workspace.refresh_requested' and index > record.traceStart then requested, requested_index = event, index end
    end
  end
  -- files counts newly installed source batches; unchanged sources can all be reused.
  check('authoritative generation completed project/library/SDK indexing', accepted and indexed and scanned
    and accepted.args.failed == 0 and accepted.args.classpathComplete == true and indexed.args.failed == 0
    and indexed.args.jdkComplete == true and type(scanned.args.files) == 'number'
    and scanned.args.files % 1 == 0 and scanned.args.files >= 0, { accepted = accepted, indexed = indexed, scanned = scanned })
  check('completed model evidence precedes the command result', scanned_index < accepted_index
    and accepted_index <= indexed_index and indexed_index < selected_index, { scanned_index, accepted_index, indexed_index, selected_index })
  check('command actually read selected project inputs and root inventory', operations.projectInputs == true and operations.rootFileInventory == true, operations)
  if refresh then
    check('this command requested a fresh real refresh generation', requested ~= nil and scanned.args.forcedModel == true, { requested, scanned })
    check('refresh request precedes that generation completion', requested_index < scanned_index, { requested_index, scanned_index })
    check('host supplied the completed refreshGeneration, independently visible in command trace',
      type(state.refresh_generation) == 'string' and #state.refresh_generation > 0, state)
    exact('refreshGeneration is associated with that completed host generation', state.refresh_generation, generation)
  end
  record.authority = { command = selected, accepted = accepted, indexed = indexed, scanned = scanned, requested = requested, factOps = operations }
  return generation
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
local function positive_request(label, command, payload, refresh)
  local value, err, record = request(label, command, payload, buffers.patch)
  check(label .. ' transport succeeded', err == nil, err)
  local frame = proof.frames[#proof.frames]
  local outer = frame and frame.params and frame.params.arguments and frame.params.arguments[1]
  check(label .. ' uses the actual provider route', frame and frame.params.command == 'ktlsp.providerCommand'
    and outer and outer.provider == provider and outer.command == command, frame)
  if refresh then exact(label .. ' requests host refresh', outer.refreshProject, true) end
  exact(label .. ' is the actual received envelope', frame.result, value)
  positive.ready(record, refresh)
  return value, record
end
local function project_snapshot(label, configs)
  local dump, dump_record = positive_request(label .. ' dump', 'mcdev.dumpContext', domain_payload(buffers.patch, { 1, 0 }))
  local model = positive.dump(dump, configs)
  local info, info_record = positive_request(label .. ' info', 'mcdev.info', domain_payload(buffers.patch, { 1, 0 }))
  positive.info(info, model)
  exact(label .. ' project projections share the accepted generation', info_record.authority.command.args.analysis_generation,
    dump_record.authority.command.args.analysis_generation)
  proof.projectSnapshots = proof.projectSnapshots or {}
  local snapshot = { label = label, dump = vim.deepcopy(dump), info = vim.deepcopy(info),
    dumpAuthority = dump_record.authority, infoAuthority = info_record.authority }
  proof.projectSnapshots[#proof.projectSnapshots + 1] = snapshot
  return snapshot
end
local function fixture_change(path, content, kind)
  check('refresh mutation stays in the isolated resource fixture', positive.path(path):sub(1, #positive.path(project) + 1) == positive.path(project) .. '/', path)
  local before = vim.uv.fs_stat(path) and vim.fn.sha256(positive.read(path)) or vim.NIL
  if content then
    local file = assert(io.open(path, 'wb')); assert(file:write(content)); assert(file:close())
  else assert(os.remove(path)) end
  check('resource watch notification was sent', client:notify('workspace/didChangeWatchedFiles',
    { changes = {{ uri = vim.uri_from_fname(path), type = kind }} }), path)
  proof.refreshFixtureChanges = proof.refreshFixtureChanges or {}
  proof.refreshFixtureChanges[#proof.refreshFixtureChanges + 1] = {
    documentUri = vim.uri_from_fname(path), kind = kind, beforeSha256 = before,
    afterSha256 = content and vim.fn.sha256(positive.read(path)) or vim.NIL, at = vim.uv.hrtime(),
  }
end

local completion
local ok, failure = xpcall(function()
  positive.load()
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
  run_case('restore exact oracle Java inputs before positive commands', nil, function()
    local floor = acceptance:mark()
    vim.api.nvim_buf_set_lines(buffers.owner, 0, -1, false, vim.fn.readfile(paths.owner))
    vim.bo[buffers.owner].modified = false
    flush(); await_accepted(buffers.owner, floor)
    proof.oracleBuffers = {}
    for _, key in ipairs({ 'owner', 'patch', 'target', 'expression' }) do
      local relative = key == 'expression' and 'src/main/java/fixture/ExampleMixin.java'
        or 'src/main/java/fixture/' .. ({ owner = 'Owner', patch = 'Patch', target = 'Target' })[key] .. '.java'
      local hash = vim.fn.sha256(full_text(buffers[key]))
      exact('current oracle buffer SHA ' .. key, hash, positive.oracle.inputSha256[relative])
      proof.oracleBuffers[key] = { documentUri = uri(buffers[key]), sourceSha256 = hash,
        documentVersion = vim.lsp.util.buf_versions[buffers[key]] }
    end
    return proof.oracleBuffers
  end)
  run_case('full positive references DTO', 'mcdev.references', function()
    local payload = domain_payload(buffers.patch, absolute_cursor(buffers.patch, 1, 'Target'))
    exact('positive reference cursor matches original capture', payload.context.position, positive.oracle.requests.references.position)
    local value = positive_request('positive references', 'mcdev.references', payload)
    positive.references(value)
    return value
  end)
  local baseline
  run_case('full positive dumpContext DTO', 'mcdev.dumpContext', function()
    local value, record = positive_request('baseline dump', 'mcdev.dumpContext', domain_payload(buffers.patch, { 1, 0 }))
    positive.dump(value, { project .. '/src/main/resources/fixture.mixins.json' })
    baseline = { label = 'baseline', dump = vim.deepcopy(value), dumpAuthority = record.authority }
    proof.projectSnapshots = { baseline }
    return baseline.dump
  end)
  run_case('full positive info DTO', 'mcdev.info', function()
    check('baseline model exists for full Info comparison', baseline ~= nil, baseline)
    local value, record = positive_request('baseline info', 'mcdev.info', domain_payload(buffers.patch, { 1, 0 }))
    positive.info(value, baseline.dump.result)
    exact('baseline project projections share the accepted generation', record.authority.command.args.analysis_generation,
      baseline.dumpAuthority.command.args.analysis_generation)
    baseline.info, baseline.infoAuthority = vim.deepcopy(value), record.authority
    return baseline.info
  end)
  run_case('full positive Mixin registration code action DTO', 'mcdev.codeAction', function()
    local config = project .. '/src/main/resources/fixture.mixins.json'
    exact('config has the exact original oracle input', positive.read(config), positive.oracle.configBefore)
    local start = positive.oracle.requests.codeAction.range.start
    local payload = domain_payload(buffers.patch, { start.line + 1, start.character })
    payload.range = vim.deepcopy(positive.oracle.requests.codeAction.range)
    payload.diagnosticCodes = vim.deepcopy(positive.oracle.requests.codeAction.diagnosticCodes)
    local value = positive_request('positive registration fix', 'mcdev.codeAction', payload)
    positive.envelope('positive registration fix', value, { 'codeAction' })
    exact('full original quickfix DTO and UTF-16 edits', value.result, positive.oracle_result('codeAction'))
    local bufnr = vim.fn.bufadd(config); vim.fn.bufload(bufnr)
    check('config edit probe does not change host resource authority', not vim.lsp.buf_is_attached(bufnr, client.id), bufnr)
    local before = full_text(bufnr)
    local passed, applied = xpcall(function()
      local changes = {}
      for _, workspace in ipairs(value.result.actions[1].edits) do changes[workspace.documentUri] = vim.deepcopy(workspace.edits) end
      vim.lsp.util.apply_workspace_edit({ changes = changes }, 'utf-16')
      exact('real editor applies original insertion without reformatting', full_text(bufnr), positive.oracle.configAfter)
      exact('edited config retains package and lists Patch once', vim.json.decode(full_text(bufnr)), { package = 'fixture', mixins = { 'Patch' } })
      exact('workspace edit leaves disk input unchanged', positive.read(config), positive.oracle.configBefore)
      proof.appliedConfig = { documentUri = vim.uri_from_bufnr(bufnr), text = full_text(bufnr),
        sourceSha256 = vim.fn.sha256(full_text(bufnr)), diskUnchanged = true, modified = vim.bo[bufnr].modified }
      return proof.appliedConfig
    end, debug.traceback)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, vim.split(before, '\n', { plain = true }))
    vim.bo[bufnr].modified = false
    if not passed then error(applied) end
    exact('config scratch buffer is restored exactly', full_text(bufnr), positive.oracle.configBefore)
    return { response = value, applied = applied }
  end)
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
  local extra_a = project .. '/src/main/resources/reindex-proof.mixins.json'
  local extra_b = project .. '/src/main/resources/reload-proof.mixins.json'
  local reindexed
  run_case('observable positive reindex refresh', 'mcdev.reindex', function()
    check('baseline snapshot exists before refresh', baseline ~= nil, baseline)
    check('new reindex config path is not preexisting', vim.uv.fs_stat(extra_a) == nil, extra_a)
    fixture_change(extra_a, '{"package":"fixture","mixins":[]}', 1)
    local value, record = positive_request('reindex after adding config', 'mcdev.reindex', domain_payload(buffers.patch, { 1, 0 }), true)
    local result = positive.envelope('positive reindex', value, { 'reindex' })
    positive.shape('reindex result', result, { 'status', 'indexState', 'classpathEntries' })
    exact('reindex original status', result.status, 'reindex complete')
    reindexed = project_snapshot('after reindex', { project .. '/src/main/resources/fixture.mixins.json', extra_a })
    exact('post-reindex model belongs to the completed refresh generation', reindexed.dumpAuthority.command.args.analysis_generation,
      record.authority.command.args.analysis_generation)
    exact('real config discovery changed 1 to 2', { #baseline.dump.result.mixinConfigs, #reindexed.dump.result.mixinConfigs }, { 1, 2 })
    exact('reindex state equals accepted post-refresh dump', result.indexState, reindexed.dump.result.indexState)
    exact('reindex count equals actual post-refresh inventory', result.classpathEntries, reindexed.dump.result.classpath.entryCount)
    check('fresh refresh generation differs from baseline token', tostring(record.authority.command.args.analysis_generation)
      ~= tostring(baseline.dumpAuthority.command.args.analysis_generation), record.authority)
    proof.reindexEffect = { before = baseline, after = reindexed, refresh = value, authority = record.authority }
    return proof.reindexEffect
  end)
  run_case('observable positive reloadProjectContext refresh', 'mcdev.reloadProjectContext', function()
    check('reindex snapshot exists before distinct reload mutation', reindexed ~= nil, reindexed)
    check('new reload config path is not preexisting', vim.uv.fs_stat(extra_b) == nil, extra_b)
    fixture_change(extra_a, nil, 3)
    fixture_change(extra_b, '{"package":"fixture","mixins":[]}', 1)
    local value, record = positive_request('reload after replacing config', 'mcdev.reloadProjectContext', domain_payload(buffers.patch, { 1, 0 }), true)
    local result = positive.envelope('positive reload', value, { 'reloadProjectContext', 'dumpContext', 'info', 'reindex' })
    positive.shape('reload result', result, { 'status', 'indexState', 'classpathEntries', 'lines' })
    exact('reload original status', result.status, 'project context reloaded')
    local reloaded = project_snapshot('after reload', { project .. '/src/main/resources/fixture.mixins.json', extra_b })
    exact('post-reload model belongs to the completed refresh generation', reloaded.dumpAuthority.command.args.analysis_generation,
      record.authority.command.args.analysis_generation)
    exact('reload state equals accepted post-refresh dump', result.indexState, reloaded.dump.result.indexState)
    exact('reload count equals actual post-refresh inventory', result.classpathEntries, reloaded.dump.result.classpath.entryCount)
    exact('every reload project info line matches the accepted post-refresh model', result.lines, reloaded.dump.result.lines)
    check('reload issued a distinct refresh token', tostring(record.authority.command.args.analysis_generation)
      ~= tostring(reindexed.dumpAuthority.command.args.analysis_generation), record.authority)
    proof.reloadEffect = { before = reindexed, after = reloaded, refresh = value, authority = record.authority }
    return proof.reloadEffect
  end)
  run_case('restore isolated refresh fixtures and lifecycle input', nil, function()
    -- All exact project/config baseline cases ran before these reversible probes.
    for _, path in ipairs({ extra_a, extra_b }) do if vim.uv.fs_stat(path) then fixture_change(path, nil, 3) end end
    check('both temporary configs were removed', vim.uv.fs_stat(extra_a) == nil and vim.uv.fs_stat(extra_b) == nil, { extra_a, extra_b })
    local floor = acceptance:mark()
    vim.api.nvim_buf_set_lines(buffers.owner, 3, 4, false, { '// 未保存' })
    vim.api.nvim_buf_set_lines(buffers.owner, 6, 7, false, { '    /* 😀 */ @Inject(meth)' })
    flush()
    return await_accepted(buffers.owner, floor)
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
  ['restore exact oracle Java inputs before positive commands'] = true,
  ['full positive references DTO'] = true,
  ['full positive Mixin registration code action DTO'] = true,
  ['full positive dumpContext DTO'] = true,
  ['full positive info DTO'] = true,
  ['observable positive reindex refresh'] = true,
  ['observable positive reloadProjectContext refresh'] = true,
  ['restore isolated refresh fixtures and lifecycle input'] = true,
}
local proven_commands = {}
for _, row in ipairs(proof.matrix) do
  if row.status == 'passed' then
    required[row.case] = nil
    if row.command and row.featureProved then proven_commands[row.command] = true end
  end
end
local all_commands = true
for _, id in ipairs(command_ids) do if not proven_commands[id] then all_commands = false end end
proof.provenCommands = proven_commands
proof.all11CommandsVerified = all_commands and next(required) == nil
proof.scopePassed = ok and not proof.indexingIncomplete and proof.all11CommandsVerified
proof.missingRequired = required
save()
if not proof.scopePassed then
  io.stderr:write('Command scope incomplete: ' .. vim.inspect({ error = failure, missing = required }) .. '\n')
  vim.cmd('cquit 1')
end
print('command editor smoke: PASS (' .. proof.checks .. ' checks)')
vim.cmd('qa!')
