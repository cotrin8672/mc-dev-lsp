-- This mock checks synchronization/provenance only; it credits no original CEM assertion.
local repo = vim.fn.getcwd()
local saved = { dofile = dofile, writefile = vim.fn.writefile, wait = vim.wait, hrtime = vim.uv.hrtime,
  window = vim.api.nvim_get_current_win(), buffer = vim.api.nvim_get_current_buf(),
  cursor = vim.api.nvim_win_get_cursor(0), runtimepath = vim.o.runtimepath,
  virtualedit = vim.o.virtualedit, completeopt = vim.o.completeopt, record = _G.mcdev_record_assertion }
local states, tracking, bufnr = {}, nil, nil
local ok, failure = xpcall(function()
  vim.opt.runtimepath:prepend(repo .. '/mcdev-nvim')
  local readiness = dofile(repo .. '/mcdev-companion/tests/support/accepted_command_readiness.lua')
  local convert = require('mcdev.convert')
  local product_completion = require('mcdev.completion')
  saved.product_completion, saved.complete = product_completion, product_completion.complete
  saved.product_protocol = require('mcdev.protocol')
  saved.request = saved.product_protocol.request
  tracking = require('vim.lsp._changetracking')
  saved.flush = tracking.flush
  bufnr = vim.api.nvim_create_buf(false, true)
  local path = repo .. '/build/nvim-test-logs/cem-readiness-mock-' .. vim.fn.getpid() .. '/Owner.java'
  vim.api.nvim_buf_set_name(bufnr, path)
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { '// 😀', 'class Owner {}' })
  vim.api.nvim_set_current_buf(bufnr)
  local files, waits, flushes = {}, {}, 0
  local instance, version, serial, unavailable, origin_ok, helper_only = 37, 17, 0, 0, true, false
  local clock, scheduled, replies, info_requests, on_info_request = 0, {}, {}, 0, nil
  local function owner_key_for(owner)
    if owner:match('^file:') then owner = vim.uri_to_fname(owner) end
    owner = vim.fs.normalize(owner)
    return vim.fn.has('win32') == 1 and owner:lower() or owner
  end
  local owner_key = owner_key_for(path)
  local function schedule(delay, callback)
    scheduled[#scheduled + 1] = { at = clock + delay, callback = callback }
  end
  vim.uv.hrtime = function() return clock * 1e6 end
  vim.wait = function(timeout, predicate, interval)
    local deadline = clock + timeout
    while true do
      for index = #scheduled, 1, -1 do
        if scheduled[index].at <= clock then table.remove(scheduled, index).callback() end
      end
      if predicate() then return true end
      if clock >= deadline then return false end
      clock = math.min(deadline, clock + (interval or 10))
    end
  end
  local actual_trace = {
    owner_key = owner_key_for,
    start = function()
      return { events = {},
        mark = function(self) return #self.events end,
        wait_for = function(self, expected, after, _, options)
          waits[#waits + 1] = { expected = vim.deepcopy(expected), after = after, instance = options.required_instance }
          if options.required_instance and options.required_instance ~= instance then return false, {} end
          serial = serial + 1
          local event = { provider = 'mcdev.mixin', instance = instance,
            evaluation = tostring(serial + 900), owner = path, source_sha256 = expected[owner_key], publication_revision = serial + 500 }
          self.events[#self.events + 1] = event
          return true, { [owner_key] = event }
        end,
        close = function() end,
      }
    end,
  }
  local client = { id = 71 }
  local raw_items = { { label = 'native', kind = 'method', insertText = 'native()V', metadata = { owner = 'fixture/Target' } } }
  local protocol = { commands = { completion = 'mcdev.completion', info = 'mcdev.info' } }
  function protocol.context(buffer, position)
    return { documentUri = vim.uri_from_bufnr(buffer), bufferText = vim.lsp._buf_get_full_text(buffer),
      documentVersion = version, position = convert.to_lsp_position(buffer, position) }
  end
  function protocol.request(command, payload, callback)
    local err, result
    local reply
    if command == protocol.commands.info then
      info_requests = info_requests + 1
      if on_info_request then on_info_request(info_requests, payload) end
      reply = table.remove(replies, 1)
    end
    if reply then err, result = reply.error, reply.result
    elseif command == protocol.commands.info and unavailable > 0 then
      unavailable = unavailable - 1
      err = { code = -32002, data = { providerCommandReason = 'CommandUnavailable' } }
    else result = { result = command == protocol.commands.completion and { items = vim.deepcopy(raw_items) } or { lines = {} } } end
    callback(result, err, { client = client, client_id = client.id,
      route = origin_ok and 'ktlsp.providerCommand' or 'loopback', provider = 'mcdev.mixin', command = command })
    return function() end
  end
  local completion = { to_lsp_item = product_completion.to_lsp_item, item_key = product_completion.item_key }
  function completion.complete(callback, buffer, position)
    return protocol.request(protocol.commands.completion, { context = protocol.context(buffer, position) }, function(envelope)
      local items = {}
      for _, item in ipairs(envelope.result.items) do items[#items + 1] = completion.to_lsp_item(item) end
      if helper_only then items = { { label = 'helper-only', insertText = 'helper-only' } } end
      callback({ items = items, isIncomplete = false })
    end, buffer)
  end
  local function start()
    clock, scheduled, replies, info_requests, on_info_request = 0, {}, {}, 0, nil
    unavailable, origin_ok, helper_only, version = 0, true, false, 17
    local state = readiness.start({ trace = 'mock.trace', proof = 'mock-proof.json', timeout = 1 })
    states[#states + 1] = state
    state:attach(client, protocol, completion, { exited = false })
    state:log_message('ktlsp indexed 1 project files')
    state:log_message('ktlsp indexed 2 library files')
    state:wait_indexed()
    state:track(bufnr)
    return state
  end
  local function close(state)
    state:close()
    vim.api.nvim_del_autocmd(state.leave_autocmd)
  end
  local stale = { code = -32801, message = 'evaluation is Stale', data = { stage = 'prepay-input' } }
  local function fresh_after_request(state, mutate, delay)
    local event = vim.deepcopy(state.records[#state.records].accepted)
    event.evaluation = tostring(tonumber(event.evaluation) + 1)
    event.publication_revision = event.publication_revision + 1
    if mutate then mutate(event, state.records[#state.records].accepted) end
    schedule(delay or 40, function() state.acceptance.events[#state.acceptance.events + 1] = event end)
  end
  _G.dofile = function(file)
    if file:match('/accepted_trace%.lua$') then return actual_trace end
    return saved.dofile(file)
  end
  vim.fn.writefile = function(lines, file, mode)
    if file == 'mock-proof.json' or file == 'mock-proof.json.frames.jsonl' then
      if mode ~= 'a' then files[file] = {} end
      files[file] = files[file] or {}
      vim.list_extend(files[file], lines)
      return 0
    end
    return saved.writefile(lines, file, mode)
  end
  tracking.flush = function(current) assert(current == client); flushes = flushes + 1 end
  local state = start()
  local before = state:await(bufnr)
  assert(before.source_sha256 == vim.fn.sha256(vim.lsp._buf_get_full_text(bufnr)))
  assert(state.instance == 37)
  local floor = state.tracked[bufnr].after
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '// unsaved 猫 😀' })
  assert(state.tracked[bufnr].after > floor, 'each edit must retire earlier acceptance evidence')
  unavailable = 1
  state:preflight(bufnr)
  assert(#state.proof.preflight == 2 and state.proof.preflight[1].retry == 'CommandUnavailable')
  local finalized
  completion.complete(function(value) finalized = value end, bufnr, { 2, 6 })
  assert(finalized and #finalized.items == 1)
  assert(state.proof.completions[1].actualProviderReply)
  assert(state.proof.completions[1].documentVersion == version)
  assert(state.proof.commands[#state.proof.commands].acceptedEvaluation ~= tostring(version))
  assert(#state.proof.originalAssertions == 0 and not state.proof.scopePassed)
  instance = 99
  assert(not pcall(state.await, state, bufnr), 'a different provider instance cannot satisfy the current owner')
  instance = 37
  close(state)

  state = start()
  helper_only = true
  completion.complete(function() end, bufnr, { 2, 6 })
  assert(not state.proof.completions[1].actualProviderReply and state.failure:find('fallback', 1, true))
  helper_only = false
  close(state)

  state = start()
  origin_ok = false
  assert(not pcall(state.preflight, state, bufnr), 'a helper/loopback origin cannot establish provider readiness')
  close(state)

  state = start()
  replies = { { error = vim.deepcopy(stale) } }
  local initial_flushes = flushes
  on_info_request = function(index) if index == 1 then fresh_after_request(state) end end
  state:preflight(bufnr)
  local retry = state.proof.preflightRetry
  assert(info_requests == 2 and #state.proof.preflight == 2 and clock == 40)
  assert(vim.deep_equal(retry.initialError, stale) and vim.deep_equal(state.proof.preflight[1].error, stale))
  assert(retry.observedFreshAccepted and retry.before.evaluation ~= retry.after.evaluation
    and retry.before.publication_revision ~= retry.after.publication_revision)
  assert(retry.before.instance == retry.after.instance and retry.documentVersion == version)
  assert(retry.before.source_sha256 == retry.after.source_sha256 and retry.sourceSha256 == retry.after.source_sha256)
  assert(owner_key_for(retry.before.owner) == owner_key_for(retry.after.owner))
  assert(retry.beforeCommandIndex == 1 and retry.afterCommandIndex == 2)
  assert(state.proof.preflight[2].freshAcceptedRetry and state.proof.preflight[2].ready)
  assert(state.records[2].accepted.evaluation == retry.after.evaluation and flushes == initial_flushes + 1,
    'fresh retry uses the observed acceptance without synthesizing or flushing a document change')
  assert(#files['mock-proof.json.frames.jsonl'] == 2)
  local first_frame = vim.json.decode(files['mock-proof.json.frames.jsonl'][1])
  local second_frame = vim.json.decode(files['mock-proof.json.frames.jsonl'][2])
  local saved_proof = vim.json.decode(files['mock-proof.json'][1])
  assert(first_frame.error.code == -32801 and first_frame.commandIndex == 1 and first_frame.authentic)
  assert(second_frame.result.result and second_frame.commandIndex == 2 and second_frame.authentic)
  assert(saved_proof.preflightRetry.initialError.code == -32801 and saved_proof.preflight[1].error.code == -32801)
  assert(not state.failure and #state.proof.originalAssertions == 0 and not state.proof.scopePassed)
  close(state)

  -- Each rejected observation consumes only mock time and must never cause a second RPC.
  local rejected = {
    { name = 'same evaluation', mutate = function(event, before) event.evaluation = before.evaluation end },
    { name = 'same publication', mutate = function(event, before) event.publication_revision = before.publication_revision end },
    { name = 'different instance', mutate = function(event) event.instance = 99 end },
    { name = 'different owner', mutate = function(event) event.owner = path .. '.other' end },
    { name = 'different hash', mutate = function(event) event.source_sha256 = string.rep('0', 64) end },
    { name = 'no new acceptance', absent = true },
    { name = 'different editor version', version = true },
  }
  for _, variant in ipairs(rejected) do
    state = start()
    replies = { { error = vim.deepcopy(stale) } }
    on_info_request = function()
      if not variant.absent then fresh_after_request(state, variant.mutate) end
      if variant.version then schedule(20, function() version = version + 1 end) end
    end
    assert(not pcall(state.preflight, state, bufnr), variant.name .. ' must fail closed')
    assert(info_requests == 1 and #state.proof.commands == 1, variant.name .. ' cannot cause a resend')
    assert(not state.proof.preflightRetry.observedFreshAccepted and state.proof.preflightRetry.after == nil)
    assert(state.proof.preflightRetry.initialError.code == -32801 and #state.proof.preflight == 1)
    assert(clock <= 60000)
    close(state)
  end

  state = start()
  replies = { { error = vim.deepcopy(stale) }, { error = vim.deepcopy(stale) } }
  on_info_request = function(index) if index == 1 then fresh_after_request(state) end end
  assert(not pcall(state.preflight, state, bufnr), 'the second Stale must fail without another fresh retry')
  assert(info_requests == 2 and #state.proof.preflight == 2 and #state.proof.commands == 2)
  assert(state.proof.preflightRetry.initialError.code == -32801 and state.proof.preflight[2].error.code == -32801)
  assert(state.proof.preflightRetry.afterCommandIndex == 2 and state.failure:find('Stale again', 1, true))
  close(state)

  for _, err in ipairs({ { code = -32800, message = 'Cancelled' }, { code = -32602, message = 'Protocol' },
    { code = -32055, message = 'unrelated RPC error' },
    { code = -32002, data = { providerCommandReason = 'DifferentReason' } } }) do
    state = start()
    replies = { { error = vim.deepcopy(err) } }
    on_info_request = function() fresh_after_request(state) end
    assert(not pcall(state.preflight, state, bufnr), 'a non-Stale RPC error must fail immediately')
    assert(info_requests == 1 and #state.proof.preflight == 1 and state.proof.preflightRetry == nil and clock == 0)
    assert(state.proof.preflight[1].error.code == err.code)
    close(state)
  end
  assert(flushes > 0 and #waits > 0)
  assert(vim.fn.filereadable('mock-proof.json') == 0, 'mock output cannot become live-host evidence')
end, debug.traceback)
for _, state in ipairs(states) do
  pcall(state.close, state)
  pcall(vim.api.nvim_del_autocmd, state.leave_autocmd)
end
_G.dofile, vim.fn.writefile, _G.mcdev_record_assertion = saved.dofile, saved.writefile, saved.record
vim.wait, vim.uv.hrtime = saved.wait, saved.hrtime
if tracking then tracking.flush = saved.flush end
if saved.product_protocol then saved.product_protocol.request = saved.request end
if saved.product_completion then saved.product_completion.complete = saved.complete end
vim.o.runtimepath, vim.o.virtualedit, vim.o.completeopt = saved.runtimepath, saved.virtualedit, saved.completeopt
vim.api.nvim_set_current_win(saved.window)
vim.api.nvim_win_set_buf(saved.window, saved.buffer)
if bufnr and vim.api.nvim_buf_is_valid(bufnr) then vim.api.nvim_buf_delete(bufnr, { force = true }) end
vim.api.nvim_win_set_cursor(saved.window, saved.cursor)
if not ok then error(failure) end
print('CEM readiness seven mock case groups passed, including one fresh accepted retry and strict error limits; no live scenario credit')
return 7
