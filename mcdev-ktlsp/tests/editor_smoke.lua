-- Disposable source fixtures test LSP semantics; these are reduced annotation declarations, not Mixin runtime artifacts.
local project = assert(vim.env.MCDEV_SMOKE_PROJECT, 'missing disposable project path')
local artifacts = assert(vim.env.MCDEV_SMOKE_ARTIFACTS, 'missing artifact path')
local binary = assert(vim.env.MCDEV_SMOKE_BINARY, 'missing explicit server binary')
local companion = vim.env.MCDEV_SMOKE_COMPANION
local client, requests, checks = nil, {}, 0
local mutations, completion_proofs = {}, {}
local shutdown = { exited = false }
local indexing = { project = false, libraries = false, messages = {} }
local acceptance, acceptance_module
local current_instance
if companion then
  vim.env.KTLSP_TRACE = artifacts .. '/host.trace.jsonl'
  acceptance_module = dofile('mcdev-companion/tests/support/accepted_trace.lua')
  acceptance = acceptance_module.start(vim.env.KTLSP_TRACE, 'mcdev.mixin')
end
local sources = {
  ['org/spongepowered/asm/mixin/Mixin.java'] = {
    'package org.spongepowered.asm.mixin;',
    'public @interface Mixin { Class<?>[] value() default {}; String[] targets() default {}; }',
  },
  ['org/spongepowered/asm/mixin/Shadow.java'] = {
    'package org.spongepowered.asm.mixin;',
    'public @interface Shadow { String prefix() default "shadow$"; String[] aliases() default {}; }',
  },
  ['fixture/Target.java'] = {
    'package fixture;',
    'public class Target {',
    '    public long twice(long x) { return x; }',
    '    public int twice(int x) { return x; }',
    '}',
  },
  ['fixture/JavaMixin.java'] = {
    'package fixture;',
    'import org.spongepowered.asm.mixin.Mixin;',
    'import org.spongepowered.asm.mixin.Shadow;',
    '@Mixin(Target.class)',
    'abstract class JavaMixin {',
    '    @Shadow abstract int shadow$twice(int x);',
    '    int invoke(int x) { return shadow$twice(x); }',
    '}',
  },
  ['fixture/KotlinMixin.kt'] = {
    'package fixture',
    'import org.spongepowered.asm.mixin.Mixin',
    'import org.spongepowered.asm.mixin.Shadow',
    '@Mixin(Target::class)',
    'abstract class KotlinMixin {',
    '    @Shadow abstract fun `shadow$twice`(x: Int): Int',
    '    fun invoke(x: Int): Int = `shadow$twice`(x)',
    '}',
  },
  ['fixture/KotlinTarget.kt'] = {
    'package fixture',
    'class KotlinTarget {',
    '    fun triple(x: Int): Int = x',
    '}',
  },
  ['fixture/KotlinTargetMixin.kt'] = {
    'package fixture',
    'import org.spongepowered.asm.mixin.Mixin',
    'import org.spongepowered.asm.mixin.Shadow',
    '@Mixin(KotlinTarget::class)',
    'abstract class KotlinTargetMixin {',
    '    @Shadow abstract fun `shadow$triple`(x: Int): Int',
    '}',
  },
}
local buffers = {}
local function check(name, ok, detail)
  assert(ok, name .. ': ' .. vim.inspect(detail))
  checks = checks + 1
  print('PASS ' .. name)
end
local function position(file, anchor, token)
  for row, line in ipairs(sources[file]) do
    if line:find(anchor, 1, true) then
      local column = assert(line:find(token, 1, true), 'missing fixture token') - 1
      return { line = row - 1, character = column },
        { start = { line = row - 1, character = column }, ['end'] = { line = row - 1, character = column + #token } }
    end
  end
  error('missing fixture anchor ' .. anchor)
end
local function uri(file) return vim.uri_from_bufnr(buffers[file]) end
local function request(method, file, anchor, token, extra)
  local params = vim.tbl_extend('force', { textDocument = { uri = uri(file) }, position = position(file, anchor, token) }, extra or {})
  local response, err = client:request_sync(method, params, 5000, buffers[file])
  requests[#requests + 1] = { method = method, params = params, response = response, error = err }
  check(method .. ' response', response ~= nil and response.err == nil, response or err)
  return response.result
end
local function locations(result)
  if type(result) ~= 'table' then return {} end
  return result.uri and { result } or result
end
local function matches(location, file, range)
  local path = vim.fs.normalize(vim.uri_to_fname(location.uri or location.targetUri))
  local expected = vim.fs.normalize(vim.uri_to_fname(uri(file)))
  if vim.fn.has('win32') == 1 then path, expected = path:lower(), expected:lower() end
  return path == expected and vim.deep_equal(location.range or location.targetSelectionRange, range)
end
local function definition(file, anchor, token, target, target_anchor, target_token, enabled)
  local _, range = position(target, target_anchor, target_token)
  local result = locations(request('textDocument/definition', file, anchor, token))
  local found = false
  for _, location in ipairs(result) do if matches(location, target, range) then found = true end end
  check(file .. ' exact target binding enabled=' .. tostring(enabled), found == enabled, result)
end
local function diagnostics(file)
  local result = request('textDocument/diagnostic', file, 'abstract class', 'abstract')
  check(file .. ' full diagnostics', result and result.kind == 'full' and type(result.items) == 'table', result)
  for _, diagnostic in ipairs(result.items) do
    check(file .. ' valid shadow has no provider error', not diagnostic.message:find('[mcdev.mixin.', 1, true), diagnostic)
  end
end
local function provider_settings(enabled)
  return { enabled = enabled, configuration = {}, command = companion and { companion } or nil }
end
local function buffer_body(file)
  return table.concat(vim.api.nvim_buf_get_lines(buffers[file], 0, -1, false), '\n') .. '\n'
end
local function wait_accepted(files, floor, options, label)
  local expected = {}
  for _, file in ipairs(files) do expected[acceptance_module.owner_key(vim.uri_to_fname(uri(file)))] = vim.fn.sha256(buffer_body(file)) end
  local ready, found = acceptance:wait_for(expected, floor, 10000, options)
  check(label, ready, { expected = expected, found = found })
  return found
end
local function configure(enabled)
  local floor = acceptance and acceptance:mark()
  check('configuration notification', client:notify('workspace/didChangeConfiguration', {
    settings = { analysisProviders = { ['mcdev.mixin'] = provider_settings(enabled) } },
  }))
  if enabled and acceptance then
    local found = wait_accepted({ 'fixture/JavaMixin.java', 'fixture/KotlinMixin.kt', 'fixture/KotlinTargetMixin.kt' }, floor,
      { fresh_instance = true }, 'host accepted current MC source snapshots in one fresh instance')
    current_instance = select(2, next(found)).instance
    return found
  end
end
local function sync_sources()
  for file, buffer in pairs(buffers) do sources[file] = vim.api.nvim_buf_get_lines(buffer, 0, -1, false) end
end
local function typed_completion(file, anchor, name, descriptor)
  local cursor, whole = position(file, anchor, name)
  cursor.character = cursor.character + #name
  local result = request('textDocument/completion', file, anchor, name, { position = cursor, context = { triggerKind = 1 } })
  local editable = vim.deepcopy(whole)
  editable.start.character = editable.start.character + #'shadow$'
  local target_name = name:sub(#'shadow$' + 1)
  local surface_name = file:match('%.kt$') and ('`' .. name .. '`') or name
  local wanted = { label = surface_name, kind = 3, detail = 'fixture.Target ' .. descriptor,
    filterText = surface_name, sortText = surface_name, insertText = target_name, insertTextFormat = 1,
    textEdit = { range = editable, newText = target_name } }
  local found = {}
  for _, item in ipairs(result.items or result) do
    if item.label == surface_name and item.detail == wanted.detail then found[#found + 1] = item end
  end
  check(file .. ' unique typed completion', #found == 1, result)
  local item = found[1]
  for key, value in pairs(wanted) do check(file .. ' completion ' .. key, vim.deep_equal(item[key], value), item) end
  check(file .. ' completion has no hidden edits', not item.additionalTextEdits or #item.additionalTextEdits == 0, item)
  return { file = file, item = item, before = buffer_body(file), wanted = wanted }
end
local function run()
  check('server binary exists', vim.fn.executable(binary) == 1, binary)
  if companion then check('companion binary exists', vim.fn.executable(companion) == 1, companion) end
  for file, lines in pairs(sources) do
    local path = project .. '/' .. file
    vim.fn.mkdir(vim.fs.dirname(path), 'p')
    vim.fn.writefile(lines, path)
    local buffer = vim.fn.bufadd(path)
    vim.fn.bufload(buffer)
    vim.bo[buffer].filetype = file:match('%.kt$') and 'kotlin' or 'java'
    buffers[file] = buffer
  end
  local id = assert(vim.lsp.start({
    name = 'mcdev-ktlsp-smoke', cmd = { binary }, root_dir = project,
    init_options = companion and { analysisProviders = { ['mcdev.mixin'] = provider_settings(false) } } or nil,
    on_exit = function(code, signal)
      shutdown.exited, shutdown.exit_code, shutdown.signal = true, code, signal
    end,
    handlers = { ['window/logMessage'] = function(_, params)
      local message = params and params.message or ''
      if message:match('^ktlsp indexed %d+ project files') then indexing.project = true end
      if message:match('^ktlsp indexed %d+ library files') then indexing.libraries = true end
      indexing.messages[#indexing.messages + 1] = message
    end },
  }, { bufnr = buffers['fixture/JavaMixin.java'] }))
  check('initialize completed', vim.wait(20000, function()
    client = vim.lsp.get_client_by_id(id)
    return client ~= nil and client.initialized
  end, 20))
  for _, buffer in pairs(buffers) do vim.lsp.buf_attach_client(buffer, id) end
  -- initialized starts asynchronous indexing; its completion messages are the host's readiness barrier.
  check('project and library indexing completed', vim.wait(30000, function()
    return indexing.project and indexing.libraries
  end, 20), indexing)
  if companion then configure(true) end
  local cases = {
    { 'fixture/JavaMixin.java', 'abstract int shadow$twice', 'shadow$twice', 'fixture/Target.java', 'int twice(int', 'twice' },
    { 'fixture/JavaMixin.java', 'return shadow$twice', 'shadow$twice', 'fixture/Target.java', 'int twice(int', 'twice' },
    { 'fixture/KotlinMixin.kt', 'abstract fun `shadow$twice`', 'shadow$twice', 'fixture/Target.java', 'int twice(int', 'twice' },
    { 'fixture/KotlinMixin.kt', '= `shadow$twice`', 'shadow$twice', 'fixture/Target.java', 'int twice(int', 'twice' },
    { 'fixture/KotlinTargetMixin.kt', 'abstract fun `shadow$triple`', 'shadow$triple', 'fixture/KotlinTarget.kt', 'fun triple', 'triple' },
  }
  for _, case in ipairs(cases) do definition(unpack(vim.list_extend(vim.deepcopy(case), { true }))) end
  for _, file in ipairs({ 'fixture/JavaMixin.java', 'fixture/KotlinMixin.kt', 'fixture/KotlinTargetMixin.kt' }) do diagnostics(file) end
  local references = locations(request('textDocument/references', 'fixture/Target.java', 'int twice(int', 'twice', { context = { includeDeclaration = true } }))
  for _, case in ipairs(cases) do
    if case[4] == 'fixture/Target.java' then
      local token = case[1]:match('%.kt$') and ('`' .. case[3] .. '`') or case[3]
      local _, range = position(case[1], case[2], token)
      local found = false
      for _, location in ipairs(references) do if matches(location, case[1], range) then found = true end end
      check('references include exact ' .. case[1] .. ' ' .. case[2], found, references)
    end
  end
  local _, target_range = position('fixture/Target.java', 'int twice(int', 'twice')
  local target_found = false
  for _, location in ipairs(references) do
    if matches(location, 'fixture/Target.java', target_range) then target_found = true end
  end
  check('references contain exactly target int declaration and four shadow sites', #references == 5 and target_found, references)
  configure(false)
  for _, case in ipairs(cases) do definition(unpack(vim.list_extend(vim.deepcopy(case), { false }))) end
  diagnostics('fixture/JavaMixin.java')
  configure(true)
  for _, case in ipairs(cases) do definition(unpack(vim.list_extend(vim.deepcopy(case), { true }))) end
  if companion then
    local before = vim.deepcopy(sources)
    local floor = acceptance:mark()
    local edit = request('textDocument/rename', 'fixture/Target.java', 'int twice(int', 'twice', { newName = 'renamedTwice' })
    check('rename returned a workspace edit', type(edit) == 'table' and (edit.changes or edit.documentChanges), edit)
    local edits = 0
    for _, changes in pairs(edit.changes or {}) do edits = edits + #changes end
    for _, change in ipairs(edit.documentChanges or {}) do edits = edits + #(change.edits or {}) end
    check('rename edits exactly target plus four projected sites', edits == 5, edit)
    vim.lsp.util.apply_workspace_edit(vim.deepcopy(edit), client.offset_encoding)
    sync_sources()
    check('rename leaves the long overload untouched', sources['fixture/Target.java'][3] == before['fixture/Target.java'][3], sources['fixture/Target.java'])
    check('rename changed the exact int declaration', sources['fixture/Target.java'][4] == '    public int renamedTwice(int x) { return x; }', sources['fixture/Target.java'])
    check('Java prefix preserved in declaration and call', sources['fixture/JavaMixin.java'][6] == '    @Shadow abstract int shadow$renamedTwice(int x);'
      and sources['fixture/JavaMixin.java'][7] == '    int invoke(int x) { return shadow$renamedTwice(x); }', sources['fixture/JavaMixin.java'])
    check('Kotlin prefix and backticks preserved in declaration and call', sources['fixture/KotlinMixin.kt'][6] == '    @Shadow abstract fun `shadow$renamedTwice`(x: Int): Int'
      and sources['fixture/KotlinMixin.kt'][7] == '    fun invoke(x: Int): Int = `shadow$renamedTwice`(x)', sources['fixture/KotlinMixin.kt'])
    local accepted = wait_accepted({ 'fixture/Target.java', 'fixture/JavaMixin.java', 'fixture/KotlinMixin.kt' }, floor, { required_instance = current_instance }, 'rename source hashes accepted in fresh evaluations')
    mutations[#mutations + 1] = { phase = 'appliedRename', before = before, after = vim.deepcopy(sources), workspaceEdit = edit, accepted = accepted }
    for _, file in ipairs({ 'fixture/JavaMixin.java', 'fixture/KotlinMixin.kt' }) do
      definition(file, file:match('%.kt$') and 'abstract fun' or 'abstract int', 'shadow$renamedTwice', 'fixture/Target.java', 'int renamedTwice(int', 'renamedTwice', true)
    end
    before = vim.deepcopy(sources)
    floor = acceptance:mark()
    local changed = vim.deepcopy(sources['fixture/JavaMixin.java'])
    changed[6] = '    @Shadow abstract long shadow$twice(long x);'
    changed[7] = '    long invoke(long x) { return shadow$twice(x); }'
    vim.api.nvim_buf_set_lines(buffers['fixture/JavaMixin.java'], 0, -1, false, changed)
    sync_sources()
    check('Java change remains unsaved', vim.bo[buffers['fixture/JavaMixin.java']].modified, nil)
    check('unsaved disk source retains original int declaration', vim.fn.readfile(project .. '/fixture/JavaMixin.java')[6] == '    @Shadow abstract int shadow$twice(int x);', nil)
    accepted = wait_accepted({ 'fixture/JavaMixin.java' }, floor, { required_instance = current_instance }, 'unsaved long source hash accepted in fresh evaluation')
    mutations[#mutations + 1] = { phase = 'unsavedLongOverload', before = before, after = vim.deepcopy(sources), accepted = accepted }
    definition('fixture/JavaMixin.java', 'abstract long', 'shadow$twice', 'fixture/Target.java', 'long twice(long', 'twice', true)
    definition('fixture/JavaMixin.java', 'return shadow$twice', 'shadow$twice', 'fixture/Target.java', 'long twice(long', 'twice', true)
    local int_refs = locations(request('textDocument/references', 'fixture/Target.java', 'int renamedTwice(int', 'renamedTwice', { context = { includeDeclaration = true } }))
    local long_refs = locations(request('textDocument/references', 'fixture/Target.java', 'long twice(long', 'twice', { context = { includeDeclaration = true } }))
    check('old int family has only target and two Kotlin sites', #int_refs == 3, int_refs)
    check('new long family has target and two unsaved Java sites', #long_refs == 3, long_refs)
    for _, anchor in ipairs({ 'abstract fun', '= `shadow$renamedTwice`' }) do
      local _, range = position('fixture/KotlinMixin.kt', anchor, '`shadow$renamedTwice`')
      local found = false
      for _, location in ipairs(int_refs) do if matches(location, 'fixture/KotlinMixin.kt', range) then found = true end end
      check('int references preserve exact Kotlin site ' .. anchor, found, int_refs)
    end
    for _, family in ipairs({ { int_refs, 'int renamedTwice(int', 'renamedTwice' }, { long_refs, 'long twice(long', 'twice' } }) do
      local _, range = position('fixture/Target.java', family[2], family[3])
      local found = false
      for _, location in ipairs(family[1]) do if matches(location, 'fixture/Target.java', range) then found = true end end
      check('references preserve exact overload target ' .. family[2], found, family[1])
    end
    for _, case in ipairs({ { 'abstract long', 'shadow$twice' }, { 'return shadow$twice', 'shadow$twice' } }) do
      local _, range = position('fixture/JavaMixin.java', case[1], case[2])
      local found = false
      for _, location in ipairs(long_refs) do if matches(location, 'fixture/JavaMixin.java', range) then found = true end end
      check('long references include exact unsaved Java site ' .. case[1], found, long_refs)
      for _, location in ipairs(int_refs) do check('int references retract Java site ' .. case[1], not matches(location, 'fixture/JavaMixin.java', range), int_refs) end
    end
    completion_proofs = {
      typed_completion('fixture/JavaMixin.java', 'abstract long', 'shadow$twice', '(J)J'),
      typed_completion('fixture/KotlinMixin.kt', 'abstract fun', 'shadow$renamedTwice', '(I)I'),
    }
    for _, proof in ipairs(completion_proofs) do
      vim.lsp.util.apply_text_edits({ vim.deepcopy(proof.item.textEdit) }, buffers[proof.file], client.offset_encoding)
      proof.after = buffer_body(proof.file)
      check(proof.file .. ' real completion edit preserves whole buffer', proof.after == proof.before, proof)
    end
  end
end

local ok, err = xpcall(run, debug.traceback)
if client then
  client:stop()
  shutdown.clean = vim.wait(5000, function() return shutdown.exited and client:is_stopped() end, 20)
    and shutdown.exit_code == 0
  if not shutdown.clean then
    client:stop(true)
    err = (ok and '' or tostring(err) .. '\n') .. 'server did not complete clean shutdown'
    ok = false
    vim.wait(2000, function() return shutdown.exited and client:is_stopped() end, 20)
  end
end
if acceptance then acceptance:close() end
vim.fn.writefile({ vim.json.encode({
  checks = checks, requests = requests, indexing = indexing, shutdown = shutdown,
  project = project, sources = sources,
  mutations = mutations, completion_proofs = completion_proofs,
  accepted = acceptance and acceptance.events or nil,
  bootstrap = { jdk_sources = vim.env.KTLSP_JDK_SRC, reduced_source_fixture = true,
    server = binary, companion = companion or vim.NIL },
  error = ok and vim.NIL or err,
}) }, artifacts .. '/editor-proof.json')
if not ok then
  io.stderr:write(tostring(err) .. '\n')
  vim.cmd('cquit 1')
end
print('editor smoke: PASS (' .. checks .. ' checks)')
vim.cmd('qa!')
