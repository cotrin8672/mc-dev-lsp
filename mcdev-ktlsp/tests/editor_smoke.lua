-- Disposable source fixtures test LSP semantics; these are reduced annotation declarations, not Mixin runtime artifacts.
local project = assert(vim.env.MCDEV_SMOKE_PROJECT, 'missing disposable project path')
local artifacts = assert(vim.env.MCDEV_SMOKE_ARTIFACTS, 'missing artifact path')
local binary = assert(vim.env.MCDEV_SMOKE_BINARY, 'missing explicit server binary')
local client, requests, checks = nil, {}, 0
local shutdown = { exited = false }
local indexing = { project = false, libraries = false, messages = {} }
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
local function configure(enabled)
  check('configuration notification', client:notify('workspace/didChangeConfiguration', {
    settings = { analysisProviders = { ['mcdev.mixin'] = { enabled = enabled, configuration = {} } } },
  }))
end
local function run()
  check('server binary exists', vim.fn.executable(binary) == 1, binary)
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
vim.fn.writefile({ vim.json.encode({
  checks = checks, requests = requests, indexing = indexing, shutdown = shutdown,
  project = project, sources = sources,
  bootstrap = { jdk_sources = vim.env.KTLSP_JDK_SRC, reduced_source_fixture = true },
  error = ok and vim.NIL or err,
}) }, artifacts .. '/editor-proof.json')
if not ok then
  io.stderr:write(tostring(err) .. '\n')
  vim.cmd('cquit 1')
end
print('editor smoke: PASS (' .. checks .. ' checks)')
vim.cmd('qa!')
