local M = {}
local function owner_key(owner)
  if owner:match('^file:') then owner = vim.uri_to_fname(owner) end
  owner = vim.fs.normalize(owner)
  return vim.fn.has('win32') == 1 and owner:lower() or owner
end

function M.start(path, provider)
  local stat = vim.uv.fs_stat(path)
  assert(not stat or stat.size == 0, 'acceptance proof requires a fresh trace')
  assert(io.open(path, 'ab')):close()
  local state = { events = {}, offset = 0, partial = '', closed = false }
  local watcher = assert(vim.uv.new_fs_event())
  local function read_new()
    if state.closed then return end
    local size = assert(vim.uv.fs_stat(path)).size
    assert(size >= state.offset, 'trace unexpectedly truncated')
    if size == state.offset then return end
    local file = assert(io.open(path, 'rb'))
    assert(file:seek('set', state.offset))
    local chunk = file:read('*a') or ''
    file:close()
    state.offset = state.offset + #chunk
    local data = state.partial .. chunk
    local consumed = 0
    for line, next_byte in data:gmatch('([^\n]*)\n()') do
      consumed = next_byte - 1
      if line:match('%S') then
        local event = vim.json.decode(line)
        if event.name == 'extension.ipc.accepted' and event.args.provider == provider then
          local args = event.args
          assert(type(args.owner) == 'string' and type(args.evaluation) == 'string')
          assert(args.instance ~= nil and args.publication_revision ~= nil)
          assert(type(args.source_sha256) == 'string' and #args.source_sha256 == 64)
          state.events[#state.events + 1] = args
        end
      end
    end
    state.partial = data:sub(consumed + 1)
  end
  assert(watcher:start(vim.fs.dirname(path), {}, vim.schedule_wrap(function(err, filename)
    if state.closed then return end
    if err then state.failure = err; return end
    if not filename or filename == vim.fs.basename(path) then
      local ok, failure = pcall(read_new)
      if not ok then state.failure = failure end
    end
  end)))
  function state:mark()
    read_new()
    assert(not self.failure, self.failure)
    return #self.events
  end
  function state:wait_for(expected, after, timeout, options)
    options = options or {}
    local prior_instances, prior_evaluations = {}, {}
    for i = 1, after do
      local event = self.events[i]
      prior_instances[event.instance] = true
      prior_evaluations[tostring(event.instance) .. '/' .. event.evaluation] = true
    end
    local found = {}
    local ready = vim.wait(timeout, function()
      assert(not self.failure, self.failure)
      local groups = {}
      for i = after + 1, #self.events do
        local event = self.events[i]
        local key = owner_key(event.owner)
        local fresh = not prior_evaluations[tostring(event.instance) .. '/' .. event.evaluation]
          and (not options.fresh_instance or not prior_instances[event.instance])
          and (not options.required_instance or event.instance == options.required_instance)
        if fresh and expected[key] == event.source_sha256 then
          groups[event.instance] = groups[event.instance] or {}
          groups[event.instance][key] = event
        end
      end
      for _, group in pairs(groups) do
        local complete, evaluations = true, {}
        for key in pairs(expected) do
          local event = group[key]
          if not event or evaluations[event.evaluation] then complete = false; break end
          evaluations[event.evaluation] = true
        end
        if complete then found = group; return true end
      end
      return false
    end, 20)
    return ready, found
  end
  function state:close()
    self.closed = true
    watcher:stop()
    watcher:close()
  end
  return state
end
M.owner_key = owner_key
return M
