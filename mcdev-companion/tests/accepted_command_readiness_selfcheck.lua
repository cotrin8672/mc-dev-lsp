-- This mock checks synchronization/provenance only; it credits no original CEM assertion.
local repo = vim.fn.getcwd()
local saved = { dofile = dofile, writefile = vim.fn.writefile,
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
  local instance, version, mark, unavailable, origin_ok, helper_only = 37, 17, 0, 0, true, false
  local owner_key = vim.fs.normalize(path)
  if vim.fn.has('win32') == 1 then owner_key = owner_key:lower() end
  local actual_trace = {
    owner_key = function() return owner_key end,
    start = function()
      return {
        mark = function() mark = mark + 1; return mark end,
        wait_for = function(_, expected, after, _, options)
          waits[#waits + 1] = { expected = vim.deepcopy(expected), after = after, instance = options.required_instance }
          if options.required_instance and options.required_instance ~= instance then return false, {} end
          return true, { [owner_key] = { provider = 'mcdev.mixin', instance = instance,
            evaluation = tostring(mark + 900), owner = path, source_sha256 = expected[owner_key], publication_revision = 500 } }
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
  function protocol.request(command, _, callback)
    local err, result
    if command == protocol.commands.info and unavailable > 0 then
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
  assert(flushes > 0 and #waits > 0)
  assert(vim.fn.filereadable('mock-proof.json') == 0, 'mock output cannot become live-host evidence')
end, debug.traceback)
for _, state in ipairs(states) do
  pcall(state.close, state)
  pcall(vim.api.nvim_del_autocmd, state.leave_autocmd)
end
_G.dofile, vim.fn.writefile, _G.mcdev_record_assertion = saved.dofile, saved.writefile, saved.record
if tracking then tracking.flush = saved.flush end
if saved.product_protocol then saved.product_protocol.request = saved.request end
if saved.product_completion then saved.product_completion.complete = saved.complete end
vim.o.runtimepath, vim.o.virtualedit, vim.o.completeopt = saved.runtimepath, saved.virtualedit, saved.completeopt
vim.api.nvim_set_current_win(saved.window)
vim.api.nvim_win_set_buf(saved.window, saved.buffer)
if bufnr and vim.api.nvim_buf_is_valid(bufnr) then vim.api.nvim_buf_delete(bufnr, { force = true }) end
vim.api.nvim_win_set_cursor(saved.window, saved.cursor)
if not ok then error(failure) end
print('CEM readiness source hash, edits, provider instance, retry and fallback provenance selfcheck passed; no live scenario credit')
return 3
