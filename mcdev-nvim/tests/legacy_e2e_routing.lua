local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")
local ktlsp = require("mcdev.ktlsp")
local repo = vim.fn.getcwd()
local root = vim.fs.normalize(repo .. "/build/nvim-test-logs/legacy-e2e-routing-" .. vim.fn.getpid())
local saved = {
  start = vim.lsp.start_client, start_or_attach = ktlsp.start_or_attach,
  get_clients = vim.lsp.get_clients, get_client = vim.lsp.get_client_by_id,
  attach = vim.lsp.buf_attach_client, cmd = vim.cmd, dofile = dofile,
  writefile = vim.fn.writefile, options = vim.deepcopy(require("mcdev.config").options),
  current = vim.api.nvim_get_current_buf(), record = _G.mcdev_record_assertion, print = print,
}
local environment = { "MCDEV_E2E_COMPANION", "MCDEV_E2E_KTLSP", "MCDEV_E2E_WORKSPACE", "MCDEV_E2E_FIXTURE" }
for _, name in ipairs(environment) do saved[name] = vim.env[name] end
local source = { "@Mixin(SimpleTarget.class)", "class ExampleMixin {}" }
local buffers = {}
local function document(relative)
  local path = root .. "/" .. relative
  vim.fn.mkdir(vim.fs.dirname(path), "p")
  vim.fn.writefile(source, path)
  local bufnr = vim.fn.bufadd(path)
  vim.fn.bufload(bufnr)
  vim.bo[bufnr].filetype = "java"
  vim.bo[bufnr].fileformat = "dos"
  vim.api.nvim_buf_set_lines(bufnr, 1, 1, false, { "// unsaved 猫 🙂" })
  buffers[#buffers + 1] = bufnr
  return bufnr
end
local bundle_bufnr = document("src/main/java/io/github/cotrin8672/cem/mixin/DrillBlockMixin.java")
document("src/main/java/io/github/cotrin8672/cem/mixin/PlacementOffsetMixin.java")
local loom_bufnr = document("src/main/java/com/example/mixin/ExampleMixin.java")
local mapped_bufnr = document("mapped-sources/com/example/target/SimpleTarget.java")
local decoy_bufnr = document("other/Decoy.java")
local client, foreign, expected_bufnr, captured, kotlin_handoff, quit
local calls, events, cancelled = {}, {}, 0
local response_error, stalled = nil, false
local provider = true
local function make_client(config, id)
  return {
    id = id, name = "ktlsp", config = config, initialized = true, attached_buffers = {},
    server_capabilities = { executeCommandProvider = {
      commands = provider and { "ktlsp.providerCommand" } or vim.tbl_values(protocol.commands),
    } },
    request = function(self, method, params, callback, bufnr)
      h.assert_eq(self, client, "the target document must select its host, even with another buffer current")
      h.assert_eq(method, "workspace/executeCommand")
      h.assert_eq(bufnr, expected_bufnr)
      h.assert_true(self.attached_buffers[bufnr], "document attach must precede every command, including reload")
      local command = provider and params.arguments[1].command or params.command
      local payload = provider and params.arguments[1].arguments[1] or params.arguments[1]
      h.assert_eq(payload.context.documentUri, vim.uri_from_bufnr(bufnr))
      if provider then
        local outer = params.arguments[1]
        h.assert_eq(params.command, "ktlsp.providerCommand")
        h.assert_eq(outer.provider, "mcdev.mixin")
        h.assert_eq(outer.context.workspaceRoot, root)
        h.assert_eq(outer.context.documentUri, vim.uri_from_bufnr(bufnr))
        h.assert_eq(outer.context.bufferText, vim.lsp._buf_get_full_text(bufnr))
        h.assert_eq(outer.context.documentVersion, 47)
        h.assert_eq(payload.context.bufferText, outer.context.bufferText)
        h.assert_eq(payload.context.documentVersion, outer.context.documentVersion)
        local refresh = command == protocol.commands.reindex or command == protocol.commands.reload_project_context
        h.assert_eq(outer.refreshProject, refresh and true or nil)
      else
        h.assert_true(params.command:match("^mcdev%.") ~= nil, "legacy clients keep direct command routing")
        h.assert_eq(payload.context.documentVersion, vim.api.nvim_buf_get_changedtick(bufnr))
      end
      calls[#calls + 1] = { command = command, payload = payload }
      events[#events + 1] = command
      if not stalled then
        local result
        if command == protocol.commands.info then
          result = { result = { lines = { "Fabric project" } } }
        elseif command == protocol.commands.definition then
          result = { result = { locations = { {
            metadata = { kind = "class", owner = "com/example/target/SimpleTarget" },
            resolution = "source", documentUri = vim.uri_from_bufnr(mapped_bufnr),
          } } } }
        end
        callback(response_error, result)
      end
      return true, 41
    end,
    cancel_request = function(_, id) h.assert_eq(id, 41); cancelled = cancelled + 1 end,
  }
end
local function start(config)
  client = make_client(config, 731)
  local foreign_config = vim.deepcopy(config)
  foreign_config.root_dir = root .. "/other"
  foreign = make_client(foreign_config, 732)
  foreign.attached_buffers[decoy_bufnr] = true
  return client.id
end
local ok, failure = xpcall(function()
  -- Mocked scenario assertions must never count as live-host migration evidence.
  _G.mcdev_record_assertion = function(kind, caller)
    local source = caller and (caller.source or ""):gsub("\\", "/") or ""
    if saved.record and not source:find("mcdev-nvim/tests/e2e/", 1, true) then saved.record(kind, caller) end
  end
  _G.print = function() end
  vim.env.MCDEV_E2E_COMPANION, vim.env.MCDEV_E2E_KTLSP = vim.v.progpath, vim.v.progpath
  vim.env.MCDEV_E2E_WORKSPACE, vim.env.MCDEV_E2E_FIXTURE = root, "create-drill-definition-diagnostics"
  vim.lsp.start_client = start
  vim.lsp.get_client_by_id = function(id) return client and client.id == id and client or nil end
  vim.lsp.get_clients = function(opts)
    return vim.tbl_filter(function(value)
      return not opts or ((not opts.name or opts.name == value.name)
        and (not opts.bufnr or value.attached_buffers[opts.bufnr]))
    end, { foreign, client })
  end
  vim.lsp.buf_attach_client = function(bufnr, id)
    h.assert_eq(id, client.id)
    client.attached_buffers[bufnr] = true
    vim.lsp.util.buf_versions[bufnr] = 47
    events[#events + 1] = "attach"
    return true
  end
  vim.fn.writefile = function(lines, path, flags)
    if path:sub(1, #repo + 11) == repo .. "/build/e2e-" then path = root .. "/" .. vim.fs.basename(path) end
    return saved.writefile(lines, path, flags or "")
  end
  -- Capture the real harness helpers at its existing scenario handoff; no source inspection or host process.
  _G.dofile = function(path)
    if path == repo .. "/mcdev-nvim/tests/e2e/create_drill_definition_diagnostics_e2e.lua"
      or path == repo .. "/mcdev-nvim/tests/e2e/placement_offset_completion_e2e.lua" then
      return function(helpers) captured = helpers end
    end
    if path == repo .. "/mcdev-nvim/tests/e2e/kotlin_drill_e2e.lua" then
      return function(helpers) kotlin_handoff = helpers end
    end
    return saved.dofile(path)
  end
  saved.dofile(repo .. "/mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua")
  h.assert_not_nil(captured, "bundle harness must hand its routed helpers to the original scenarios")
  h.assert_not_nil(kotlin_handoff, "bundle routing must retain the Kotlin scenario handoff")
  h.assert_eq(kotlin_handoff.workspace, root)
  h.assert_eq(kotlin_handoff.workspace_root_uri, vim.uri_from_fname(root))
  h.assert_eq(kotlin_handoff.client_id, client.id)
  h.assert_eq(kotlin_handoff.with_buffer, captured.with_buffer)
  h.assert_eq(kotlin_handoff.mcdev_command, captured.mcdev_command)
  h.assert_true(type(kotlin_handoff.find_line_and_character) == "function")
  h.assert_eq(#calls, 0, "routing mocks hand off scenarios; their contents are checked separately")
  expected_bufnr = bundle_bufnr
  captured.with_buffer(vim.api.nvim_buf_get_name(bundle_bufnr), "java", nil, function(bufnr)
    vim.api.nvim_set_current_buf(decoy_bufnr)
    local context = captured.build_context(bufnr, { 1, 8 })
    local payload = { context = context, trigger = { kind = "manual" } }
    local result, err = captured.mcdev_command(protocol.commands.info, payload, 100)
    h.assert_nil(err)
    h.assert_eq(result.result.lines[1], "Fabric project")
    h.assert_eq(calls[#calls].payload, payload, "scenario payloads stay opaque")
    h.assert_eq(context.position.line, 0)
    h.assert_eq(context.position.character, 8)
    h.assert_eq(context.client.name, "mcdev.nvim-e2e")
    response_error = { code = -32800, message = "host cancelled" }
    local _, returned_error = captured.mcdev_command(protocol.commands.info, payload, 100)
    h.assert_eq(returned_error, response_error)
    response_error, stalled = nil, true
    local settled, timeout = pcall(captured.mcdev_command, protocol.commands.info, payload, 1)
    h.assert_true(not settled and tostring(timeout):find("timed out", 1, true) ~= nil)
    h.assert_eq(cancelled, 1, "a timed out request must be cancelled")
    stalled = false
    provider = false
    client.name = "jdtls"
    client.server_capabilities.executeCommandProvider.commands = vim.tbl_values(protocol.commands)
    captured.mcdev_command(protocol.commands.info, { context = captured.build_context(bufnr, { 1, 8 }) }, 100)
  end)

  for _, use_provider in ipairs({ true, false }) do
    provider, expected_bufnr, events, calls = use_provider, loom_bufnr, {}, {}
    ktlsp.start_or_attach = function(config)
      h.assert_eq(vim.api.nvim_get_current_buf(), loom_bufnr, "Loom must open its target before starting the client")
      local id = start(assert(ktlsp.extend_config(config)))
      vim.lsp.buf_attach_client(loom_bufnr, id)
      vim.api.nvim_set_current_buf(decoy_bufnr)
      return id
    end
    vim.cmd = function(command) h.assert_eq(command, "qa!"); quit = true end
    quit = false
    saved.dofile(repo .. "/mcdev-nvim/tests/e2e/jdtls_loom_e2e.lua")
    h.assert_true(quit, "Loom must reach all its original definition assertions")
    h.assert_eq(events[1], "attach")
    h.assert_eq(#calls, 4)
    h.assert_eq(calls[1].command, protocol.commands.reload_project_context)
    h.assert_eq(calls[2].command, protocol.commands.reindex)
    h.assert_eq(calls[3].command, protocol.commands.info)
    h.assert_eq(calls[4].command, protocol.commands.definition)
  end
end, debug.traceback)
vim.lsp.start_client, ktlsp.start_or_attach = saved.start, saved.start_or_attach
vim.lsp.get_clients, vim.lsp.get_client_by_id = saved.get_clients, saved.get_client
vim.lsp.buf_attach_client, vim.cmd, _G.dofile = saved.attach, saved.cmd, saved.dofile
_G.mcdev_record_assertion, _G.print = saved.record, saved.print
vim.fn.writefile, require("mcdev.config").options = saved.writefile, saved.options
for _, name in ipairs(environment) do vim.env[name] = saved[name] end
vim.api.nvim_set_current_buf(saved.current)
for _, bufnr in ipairs(buffers) do
  vim.lsp.util.buf_versions[bufnr] = nil
  vim.api.nvim_buf_delete(bufnr, { force = true })
end
if not ok then error(failure) end
print("mcdev-nvim legacy E2E harness routing tests passed (mock clients; no host acceptance claim)")
