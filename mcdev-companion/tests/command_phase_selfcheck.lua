-- Exercise the real harness helpers with a 5ms injected deadline and no host processes.
local repo = vim.fn.getcwd()
local assertions = 0
local function check(condition, message)
  assertions = assertions + 1
  assert(condition, message)
end

local function harness(options)
  options = options or {}
  local state = { waits = {}, sent = {}, cancelled = 0, stops = {}, accepts = 0, writes = {} }
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname() .. '.java')
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { 'class Owner {}' })
  state.buffer = bufnr
  local acceptance = {}
  function acceptance:wait_for(expected, _, timeout)
    state.accepts = state.accepts + 1
    state.acceptanceDeadline = timeout
    return options.accepted ~= false, expected
  end
  local protocol = {}
  function protocol.request(command, payload, callback, buffer)
    state.sent[#state.sent + 1] = { command = command, payload = payload, buffer = buffer }
    if options.respond ~= false then callback({ ok = true }, nil) end
    return function() state.cancelled = state.cancelled + 1 end
  end
  local mock = setmetatable({}, { __index = vim })
  mock.env = setmetatable({ MCDEV_COMMAND_PROJECT = repo, MCDEV_COMMAND_ARTIFACTS = repo .. '/build',
    MCDEV_COMMAND_HOST = 'unused-host', MCDEV_COMMAND_COMPANION = 'unused-companion' }, { __index = vim.env })
  mock.fn = setmetatable({ writefile = function(lines, path)
    state.writes[path] = vim.deepcopy(lines)
    return 0
  end }, { __index = vim.fn })
  mock.wait = function(timeout, predicate)
    state.waits[#state.waits + 1] = timeout
    return predicate() or vim.wait(5, predicate, 1)
  end
  state.client = {
    initialized = true, server_capabilities = { executeCommandProvider = { commands = { 'ktlsp.providerCommand' } } },
    request = function() return true, 73 end,
    stop = function(self, force)
      state.stops[#state.stops + 1] = force == true
      self.stopped = true
      state.serverOptions.on_exit(0, 0)
    end,
    is_stopped = function(self) return self.stopped == true end,
  }
  mock.lsp = setmetatable({ set_log_level = function() end,
    start = function(config)
      state.serverOptions = config
      if options.indexed ~= false then
        config.handlers['window/logMessage'](nil, { message = 'ktlsp indexed 1 project files' })
        config.handlers['window/logMessage'](nil, { message = 'ktlsp indexed 1 library files' })
      end
      return 17
    end,
    get_client_by_id = function(id) assert(id == 17); return state.client end,
    buf_attach_client = function(buffer, id) assert(buffer == bufnr and id == 17); return true end,
  }, { __index = vim.lsp })
  local captured, stop = nil, {}
  local environment = setmetatable({ vim = mock, print = function() end,
    dofile = function(path)
      assert(path == repo .. '/mcdev-companion/tests/support/accepted_trace.lua')
      return { start = function() return acceptance end, owner_key = function(path) return path end }
    end,
    require = function(name)
      if name == 'mcdev.protocol' then return protocol end
      assert(name == 'vim.lsp._changetracking')
      return { flush = function() end }
    end,
    xpcall = function(body) captured = body; error(stop) end,
  }, { __index = _G })
  local chunk = assert(loadfile(repo .. '/mcdev-companion/tests/command_editor_smoke.lua'))
  setfenv(chunk, environment)
  local loaded, failure = pcall(chunk)
  assert(not loaded and failure == stop and captured, 'capture helpers before the real-host body executes')
  environment.xpcall = xpcall
  -- Inspect closure upvalues instead of copying helper logic or adding a production test mode.
  local function upvalue(name, replacement, replace)
    for index = 1, debug.getinfo(captured, 'u').nups do
      local key, value = debug.getupvalue(captured, index)
      if key == name then
        if replace then debug.setupvalue(captured, index, replacement) end
        return value
      end
    end
    error('missing harness helper ' .. name)
  end
  upvalue('buffers', { owner = bufnr }, true)
  for _, name in ipairs({ 'start_server', 'stop_server', 'await_accepted', 'request', 'run_case', 'proof' }) do
    state[name] = upvalue(name)
  end
  state.exited = function() return upvalue('exited') end
  state.active_client = function() return upvalue('client') end
  return state
end

local function scenario(options, body)
  local state = harness(options)
  local passed, failure = xpcall(function() body(state) end, debug.traceback)
  vim.api.nvim_buf_delete(state.buffer, { force = true })
  assert(passed, failure)
end

scenario({}, function(state)
  local exited = state.start_server()
  check(vim.deep_equal(state.waits, { 30000, 180000 }), 'initialization and indexing have fixed separate deadlines')
  state.await_accepted(state.buffer, 0)
  check(state.accepts == 1 and state.acceptanceDeadline == 30000, 'acceptance follows successful indexing')
  local sent, id = state.client:request('workspace/executeCommand', {}, function() end, state.buffer)
  check(sent and id == 73 and state.proof.frames[1].requestId == 73, 'direct navigation can cancel its actual request ID')
  state.stop_server(exited)
  check(#state.stops == 1 and not state.stops[1] and state.active_client() == nil, 'cleanup stops only the owned client')
  check(exited.clean and state.proof.phaseFailure == nil, 'successful phases remain successful')
end)

scenario({ indexed = false }, function(state)
  local later = false
  local passed = pcall(function()
    state.run_case('initialization', nil, function() state.start_server(); later = true end)
    later = true
  end)
  check(not passed and not later and state.accepts == 0, 'indexing timeout aborts before acceptance or later cases')
  check(state.proof.indexingIncomplete and state.proof.phaseFailure ~= nil, 'indexing timeout is recorded')
  state.stop_server(state.exited())
  check(#state.stops == 1 and state.proof.shutdown[1].clean, 'initialization failure retains the real exit record for cleanup')
end)

scenario({ accepted = false }, function(state)
  state.start_server()
  local later = false
  local passed = pcall(function()
    state.run_case('acceptance', nil, function() state.await_accepted(state.buffer, 0) end)
    later = true
  end)
  check(not passed and not later, 'acceptance timeout cannot continue into source editing')
  check(state.proof.phaseFailure.phase == 'host accepted current source hash', 'acceptance failure records its phase')
  state.stop_server(state.exited())
end)

scenario({}, function(state)
  for index, command in ipairs({ 'mcdev.completion', 'mcdev.reindex', 'mcdev.reloadProjectContext' }) do
    local value, err = state.request(command, command, {}, state.buffer)
    check(value.ok and err == nil and state.proof.requests[index].completed, 'successful command keeps its result')
  end
  check(vim.deep_equal(state.waits, { 25000, 120000, 120000 }), 'refresh deadlines include host refresh and child evaluation')
  check(state.cancelled == 0 and state.proof.phaseFailure == nil, 'successful requests are never cancelled')
end)

for _, command in ipairs({ 'mcdev.completion', 'mcdev.reindex' }) do
  scenario({ respond = false }, function(state)
    state.start_server()
    local later = false
    local passed = pcall(function()
      state.run_case('injected timeout', command, function()
        state.request('injected timeout', command, {}, state.buffer)
        later = true
      end)
      later = true
    end)
    local request = state.proof.requests[1]
    check(not passed and not later, 'request timeout aborts the remaining case and later cases')
    check(#state.sent == 1 and state.cancelled == 1, 'timeout cancels the original request once without retrying')
    check(request.timedOut and not request.completed and request.cancelRequested and request.attempts == 1,
      'timeout and cancellation are recorded')
    check(state.proof.phaseFailure ~= nil and state.proof.matrix[1].status == 'failed', 'failed case is saved before abort')
    check(state.writes[repo .. '/build/editor-proof.json'] ~= nil, 'failure evidence is persisted')
    state.stop_server(state.exited())
    check(#state.stops == 1 and state.active_client() == nil, 'timeout cleanup stops the owned server')
  end)
end
print('command phase self-check: PASS (' .. assertions .. ' assertions)')
return assertions
