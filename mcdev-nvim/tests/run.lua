local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")

local mcdev = require("mcdev")
local completion = require("mcdev.completion")
local protocol = require("mcdev.protocol")
local buffer = require("mcdev.buffer")
local config = require("mcdev.config")
local blink = require("mcdev.blink")
local cmp = require("mcdev.cmp")
local convert = require("mcdev.convert")
local navigation = require("mcdev.navigation")
local diagnostics = require("mcdev.diagnostics")
local code_action = require("mcdev.code_action")
local hover = require("mcdev.hover")
local jdtls_helper = require("mcdev.jdtls")
local health = require("mcdev.health")
local lsp_adapter = require("mcdev.lsp")

mcdev.setup({
  jdtls = {
    extension_jar = "build/libs/io.github.mcdev.jdtls.jar",
  },
  insert = {
    at_target = "smart",
    mixin_class_import = true,
    inject_method_descriptor = "auto",
  },
})

helpers.assert_eq(mcdev.extension_jar(), "build/libs/io.github.mcdev.jdtls.jar")

local original_jdtls_options = vim.deepcopy(config.options.jdtls)
local mason_root = vim.fn.tempname()
local mason_jar = mason_root .. "/share/mcdev-jdtls-extension/io.github.mcdev.jdtls.jar"
vim.fn.mkdir(vim.fn.fnamemodify(mason_jar, ":h"), "p")
vim.fn.writefile({}, mason_jar)
config.options.jdtls.extension_jar = nil
config.options.jdtls.mason = {
  enabled = true,
  package = "mcdev-jdtls-extension",
  jar = "io.github.mcdev.jdtls.jar",
  root = mason_root,
}
helpers.assert_eq(mcdev.extension_jar(), mason_jar)

local jdtls_config = {
  init_options = {
    bundles = { "existing.jar" },
  },
}
helpers.assert_eq(jdtls_helper.extend_config(jdtls_config), jdtls_config)
helpers.assert_eq(#jdtls_config.init_options.bundles, 2)
helpers.assert_eq(jdtls_config.init_options.bundles[2], mason_jar)
jdtls_helper.extend_config(jdtls_config)
helpers.assert_eq(#jdtls_config.init_options.bundles, 2)

local original_lsp_start = vim.lsp.start
local started_config = nil
vim.lsp.start = function(start_opts)
  started_config = start_opts
  return 42
end
helpers.assert_eq(jdtls_helper.start_or_attach({
  root_dir = "/project",
  cmd = { "jdtls" },
  init_options = {
    bundles = { "debug.jar" },
  },
}), 42)
helpers.assert_eq(started_config.init_options.bundles[1], "debug.jar")
helpers.assert_eq(started_config.init_options.bundles[2], mason_jar)
vim.lsp.start = original_lsp_start
config.options.jdtls = original_jdtls_options

local item = completion.to_lsp_item({
  label = "setScreen(Screen): void",
  detail = "MinecraftClient",
  documentation = nil,
  filterText = "setScreen MinecraftClient Screen",
  insertText = "m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V",
  kind = "method",
  sortKey = "0200_setScreen",
  edit = nil,
  additionalEdits = {},
  metadata = {
    source = "at.member",
  },
})

helpers.assert_eq(item.label, "setScreen(Screen): void")
helpers.assert_eq(item.insertText, "m_91152_(Lnet/minecraft/client/gui/screens/Screen;)V")
helpers.assert_eq(item.filterText, "setScreen MinecraftClient Screen")
helpers.assert_true(item.label ~= item.insertText, "label must differ from insertText")

local payload = protocol.build_completion_payload(0, { 1, 5 })
helpers.assert_not_nil(payload.context)
helpers.assert_eq(payload.context.protocolVersion, protocol.VERSION)
helpers.assert_eq(protocol.commands.reload_project_context, "mcdev.reloadProjectContext")
helpers.assert_eq(protocol.commands.dump_context, "mcdev.dumpContext")
helpers.assert_eq(protocol.commands.hover, "mcdev.hover")
helpers.assert_eq(protocol.commands.diagnostics, "mcdev.diagnostics")
helpers.assert_eq(payload.trigger.kind, "manual")
helpers.assert_eq(payload.options.preferredAtTarget, "smart")
helpers.assert_eq(payload.options.mixinClassInsert, "import")
helpers.assert_eq(payload.options.injectMethodDescriptor, "auto")
helpers.assert_not_nil(payload.context.client)
helpers.assert_eq(payload.context.client.name, "mcdev.nvim")
helpers.assert_nil(payload.context.bufferText)
helpers.assert_not_nil(payload.context.bufferTextFallback)

local original_get_clients = vim.lsp.get_clients
local original_notify = vim.notify
local notify_message = nil
vim.lsp.get_clients = function()
  return {}
end
vim.notify = function(message)
  notify_message = message
end
protocol.request("mcdev.completion", payload, nil)
helpers.assert_eq(notify_message, "mcdev: no active JDT LS client for this buffer")
vim.lsp.get_clients = original_get_clients
vim.notify = original_notify

local callback_error = nil
protocol.request("mcdev.completion", payload, function(_, err)
  callback_error = err
end)
helpers.assert_eq(callback_error, "mcdev: no active JDT LS client for this buffer")

local blink_adapter = blink.source()
helpers.assert_eq(#blink_adapter:get_trigger_characters(), 5)

local completion_module = package.loaded["mcdev.completion"]
local original_complete = completion_module.complete
local blink_result = nil
local adapter_sources = {}
completion_module.complete = function(callback, _, _, opts)
  adapter_sources[#adapter_sources + 1] = opts and opts.source or nil
  callback({
    isIncomplete = true,
    items = {
      {
        label = "tick(): void",
        insertText = "tick",
        kind = "method",
        sortKey = "0200_tick",
        filterText = "tick",
        detail = "SimpleTarget",
        documentation = nil,
        edit = nil,
        additionalEdits = {},
        metadata = { source = "mixin.injectMethod" },
      },
    },
  })
end
vim.api.nvim_buf_set_lines(0, 0, -1, false, { '@Inject(method = "ti' })
local blink_cursor = { 1, #'@Inject(method = "ti' }
blink_adapter:get_completions({ bufnr = 0, cursor = blink_cursor }, function(result)
  blink_result = result
end)
helpers.assert_not_nil(blink_result)
helpers.assert_eq(blink_result.is_incomplete_forward, true)
helpers.assert_eq(blink_result.is_incomplete_backward, true)
helpers.assert_eq(#blink_result.items, 1)
helpers.assert_eq(blink_result.items[1].label, "tick(): void")
helpers.assert_eq(blink_result.items[1].insertText, "tick")
helpers.assert_eq(blink_result.items[1].kind, vim.lsp.protocol.CompletionItemKind.Value)
helpers.assert_eq(blink_result.items[1].kind_icon, "")
helpers.assert_eq(blink_result.items[1].kind_name, "Mixin")
helpers.assert_eq(blink_result.items[1].cursor_column, blink_cursor[2])
helpers.assert_eq(blink_result.items[1].textEdit.newText, "tick")
helpers.assert_eq(blink_result.items[1].textEdit.range.start.line, 0)
helpers.assert_eq(blink_result.items[1].textEdit.range.start.character, blink_cursor[2] - 2)
helpers.assert_eq(blink_result.items[1].textEdit.range["end"].line, 0)
helpers.assert_eq(blink_result.items[1].textEdit.range["end"].character, blink_cursor[2])
vim.lsp.util.apply_text_edits({ blink_result.items[1].textEdit }, vim.api.nvim_get_current_buf(), "utf-8")
helpers.assert_eq(vim.api.nvim_buf_get_lines(0, 0, 1, false)[1], '@Inject(method = "tick')
vim.bo.modified = false
helpers.assert_eq(adapter_sources[#adapter_sources], "blink")

local cmp_result = nil
local cmp_source = cmp.source()
cmp_source:complete({}, function(items)
  cmp_result = items
end)
helpers.assert_not_nil(cmp_result)
helpers.assert_eq(#cmp_result.items, 1)
helpers.assert_eq(cmp_result.items[1].insertText, "tick")
helpers.assert_eq(cmp_result.isIncomplete, true)
helpers.assert_eq(adapter_sources[#adapter_sources], "cmp")
completion_module.complete = original_complete

local commands = vim.api.nvim_get_commands({})
helpers.assert_not_nil(commands.McdevInfo)
helpers.assert_not_nil(commands.McdevReindex)
helpers.assert_not_nil(commands.McdevReloadProjectContext)
helpers.assert_not_nil(commands.McdevDumpContext)
helpers.assert_not_nil(commands.McdevHealth)
helpers.assert_not_nil(commands.McdevDebugCompletion)
helpers.assert_not_nil(commands.McdevDebugDiagnostics)
helpers.assert_not_nil(commands.McdevDiagnosticsRefresh)
helpers.assert_not_nil(commands.McdevDiagnosticsStop)
helpers.assert_not_nil(commands.McdevDiagnosticsStart)
helpers.assert_not_nil(commands.McdevDiagnosticsStatus)

helpers.assert_eq(mcdev.options().insert.at_target, "smart")
helpers.assert_eq(mcdev.options().insert.mixin_class_import, true)
helpers.assert_eq(mcdev.options().insert.inject_method_descriptor, "auto")
helpers.assert_eq(mcdev.options().completion.omnifunc, true)
helpers.assert_eq(mcdev.options().diagnostics.enabled, false)
helpers.assert_eq(mcdev.options().diagnostics.events[1], "BufWritePost")
helpers.assert_eq(diagnostics.running, false)
helpers.assert_eq(mcdev.options().standard_lsp.prefer, true)

local function has_buffer_keymap(bufnr, mode, lhs)
  for _, map in ipairs(vim.api.nvim_buf_get_keymap(bufnr, mode)) do
    if map.lhs == lhs then
      return true
    end
  end
  return false
end

helpers.assert_eq(has_buffer_keymap(0, "n", "gd"), false)
helpers.assert_eq(has_buffer_keymap(0, "n", "gr"), false)
helpers.assert_eq(has_buffer_keymap(0, "n", "<leader>ca"), false)
helpers.assert_eq(has_buffer_keymap(0, "v", "<leader>ca"), false)

local function with_named_buffer(name, filetype, lines, callback)
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, name)
  vim.bo[bufnr].filetype = filetype
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  callback(bufnr)
  vim.api.nvim_buf_delete(bufnr, { force = true })
end

with_named_buffer("/project/src/main/java/com/example/mixin/ExampleMixin.java", "java", {
  '@Inject(method = "dr',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_completion = protocol_module.completion
  local server_requests = 0
  protocol_module.completion = function(callback)
    server_requests = server_requests + 1
    if server_requests == 1 then
      callback({
        result = {
          items = {
            {
              label = "draw(String): void",
              insertText = "draw",
              kind = "method",
              sortKey = "0200_draw",
              filterText = "draw",
              detail = "SimpleTarget",
              additionalEdits = {},
              metadata = { source = "mixin.injectMethod" },
            },
          },
          debug = {},
        },
      }, nil)
    end
  end

  local first = nil
  completion.complete(function(result)
    first = result
  end, bufnr, { 1, #'@Inject(method = "dr' }, { source = "manual" })
  helpers.assert_eq(#first.items, 1)

  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dra' })
  local second = nil
  completion.complete(function(result)
    second = result
  end, bufnr, { 1, #'@Inject(method = "dra' }, { source = "manual" })
  helpers.assert_not_nil(second)
  helpers.assert_eq(#second.items, 1)
  helpers.assert_eq(second.items[1].insertText, "draw")
  helpers.assert_true(completion.last_local_prefix_cache_hit)
  helpers.assert_eq(server_requests, 2)

  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/resources/mod.accesswidener", "plaintext", {
  "accessWidener v2 named",
  "accessible class com/example/target/SimpleTarget",
}, function(bufnr)
  helpers.assert_eq(buffer.detect_file_type(bufnr), "access_widener")
  local payload = protocol.build_completion_payload(bufnr, { 2, 12 })
  helpers.assert_eq(payload.context.languageId, "accesswidener")
  helpers.assert_true(payload.context.documentUri:find("mod.accesswidener", 1, true) ~= nil)
end)

with_named_buffer("/project/src/main/resources/mod_at.cfg", "cfg", {
  "public com.example.target.SimpleTarget",
}, function(bufnr)
  helpers.assert_eq(buffer.detect_file_type(bufnr), "access_transformer")
  local payload = protocol.build_completion_payload(bufnr, { 1, 8 })
  helpers.assert_eq(payload.context.languageId, "accesstransformer")
  helpers.assert_true(payload.context.documentUri:find("mod_at.cfg", 1, true) ~= nil)
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ExampleMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  helpers.assert_nil(buffer.detect_file_type(bufnr))
  helpers.assert_true(buffer.is_mcdev_buffer(bufnr))
  local payload = protocol.build_completion_payload(bufnr, { 1, 8 })
  helpers.assert_eq(payload.context.languageId, "java")
  require("mcdev.attach").setup(bufnr)
  helpers.assert_eq(vim.bo[bufnr].omnifunc, "v:lua.require'mcdev.omnifunc'.complete")
end)

with_named_buffer("/project/src/main/java/com/example/mixin/NoOmnifuncMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local original_completion_options = vim.deepcopy(config.options.completion)
  config.options.completion.omnifunc = false
  require("mcdev.attach").setup(bufnr)
  helpers.assert_eq(vim.bo[bufnr].omnifunc, "")
  config.options.completion = original_completion_options
end)

with_named_buffer("/project/src/main/resources/mod.aw", "plaintext", {
  "accessWidener v2 named",
}, function(bufnr)
  helpers.assert_eq(buffer.detect_file_type(bufnr), "access_widener")
  helpers.assert_eq(buffer.effective_language_id(bufnr), "accesswidener")
  local payload = protocol.build_completion_payload(bufnr, { 1, 1 })
  helpers.assert_eq(payload.context.languageId, "accesswidener")
end)

with_named_buffer("/other-project/src/main/java/com/example/mixin/ExampleMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  vim.lsp.get_clients = function(opts)
    if opts and opts.bufnr == bufnr then
      return {
        {
          name = "jdtls",
          config = { root_dir = "/other-project" },
        },
      }
    end
    return {
      {
        name = "jdtls",
        config = { root_dir = "/current-project" },
      },
    }
  end
  local ctx = protocol.context(bufnr, { 1, 1 })
  helpers.assert_true(ctx.workspaceRoot:find("other%-project", 1, false) ~= nil, ctx.workspaceRoot)
  vim.lsp.get_clients = original_get_clients
end)

with_named_buffer("/project/src/main/resources/mod.accesswidener", "accesswidener", {
  "accessWidener v2 named",
  "acc",
}, function(bufnr)
  local requested_command = nil
  local requested_bufnr = nil
  local fallback_client = {
    name = "jdtls",
    config = { root_dir = "/project" },
    request = function(method, params, callback, request_bufnr)
      requested_command = params.command
      requested_bufnr = request_bufnr
      callback(nil, { result = { items = {} } })
    end,
  }
  vim.lsp.get_clients = function(opts)
    if opts and opts.bufnr == bufnr then
      return {}
    end
    if opts and opts.name == "jdtls" then
      return { fallback_client }
    end
    return {}
  end

  local ctx = protocol.context(bufnr, { 2, 4 })
  helpers.assert_true(ctx.workspaceRoot:find("/project", 1, true) ~= nil, ctx.workspaceRoot)
  helpers.assert_eq(protocol.active_jdtls_client(bufnr), fallback_client)

  local request_result = nil
  protocol.request("mcdev.completion", { context = ctx }, function(result, err)
    helpers.assert_nil(err)
    request_result = result
  end, bufnr)
  helpers.assert_not_nil(request_result)
  helpers.assert_eq(requested_command, "mcdev.completion")
  helpers.assert_eq(requested_bufnr, bufnr)
  vim.lsp.get_clients = original_get_clients
end)

with_named_buffer("/project/src/main/java/com/example/mixin/DiagnosticsMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_diagnostics = protocol_module.diagnostics
  local requests = 0
  protocol_module.diagnostics = function(_, _, callback)
    requests = requests + 1
    callback({ result = { diagnostics = {} } }, nil)
  end

  vim.api.nvim_buf_call(bufnr, function()
    vim.cmd("doautocmd TextChangedI")
  end)
  helpers.assert_eq(requests, 0)

  diagnostics.start({ events = { "TextChangedI" }, debounce_ms = 1, insert_mode = false })
  local original_get_mode = vim.api.nvim_get_mode
  vim.api.nvim_get_mode = function()
    return { mode = "i" }
  end
  vim.api.nvim_buf_call(bufnr, function()
    vim.cmd("doautocmd TextChangedI")
  end)
  vim.api.nvim_get_mode = original_get_mode
  vim.wait(30)
  helpers.assert_eq(requests, 0)

  diagnostics.stop()
  diagnostics.start({ events = { "BufWritePost" }, debounce_ms = 1, insert_mode = false })
  vim.api.nvim_buf_call(bufnr, function()
    vim.cmd("doautocmd BufWritePost")
  end)
  vim.wait(30)
  helpers.assert_eq(requests, 1)
  diagnostics.stop()
  protocol_module.diagnostics = original_diagnostics
end)

with_named_buffer("/project/META-INF/accesstransformer.cfg", "plaintext", {
  "public com.example.target.SimpleTarget",
}, function(bufnr)
  helpers.assert_eq(buffer.detect_file_type(bufnr), "access_transformer")
  helpers.assert_eq(buffer.effective_language_id(bufnr), "accesstransformer")
end)

with_named_buffer("/project/mod.at", "java", {
  "public com.example.target.SimpleTarget",
}, function(bufnr)
  helpers.assert_eq(buffer.detect_file_type(bufnr), "access_transformer")
  helpers.assert_eq(buffer.effective_language_id(bufnr), "accesstransformer")
end)

with_named_buffer("/project/src/main/resources/mod.accesswidener", "accesswidener", {
  "accessWidener v2 named",
  "accessible class com/example/target/SimpleTarget",
}, function(bufnr)
  local ctx = protocol.context(bufnr, { 2, 12 })
  helpers.assert_eq(ctx.position.line, 1)
  helpers.assert_eq(ctx.position.character, 12)
  helpers.assert_eq(ctx.documentVersion, vim.api.nvim_buf_get_changedtick(bufnr))
  helpers.assert_true(ctx.bufferText:find("accessible class", 1, true) ~= nil)
end)

local lsp_location = convert.to_lsp_location({
  documentUri = "file:///Mixin.java",
  resolution = "source",
  range = {
    start = { line = 1, character = 2 },
    ["end"] = { line = 1, character = 10 },
  },
})
helpers.assert_eq(lsp_location.uri, "file:///Mixin.java")
helpers.assert_eq(lsp_location.resolution, "source")

local unresolved_location = convert.to_lsp_location({
  documentUri = "",
  resolution = "unresolved",
  resolutionMessage = "no project source file",
  range = {
    start = { line = 0, character = 0 },
    ["end"] = { line = 0, character = 0 },
  },
}, "file:///Mixin.java")
helpers.assert_eq(unresolved_location.uri, "file:///Mixin.java")
helpers.assert_eq(unresolved_location.resolutionMessage, "no project source file")
helpers.assert_eq(lsp_location.range.start.line, 1)
helpers.assert_eq(lsp_location.range["end"].character, 10)

local vim_diagnostic = convert.to_vim_diagnostic({
  code = "AW_UNRESOLVED_CLASS",
  severity = "error",
  message = "Unresolved class",
  range = {
    start = { line = 1, character = 4 },
    ["end"] = { line = 1, character = 20 },
  },
  metadata = { target = "Missing" },
})
helpers.assert_eq(vim_diagnostic.lnum, 1)
helpers.assert_eq(vim_diagnostic.col, 4)
helpers.assert_eq(vim_diagnostic.code, "AW_UNRESOLVED_CLASS")
helpers.assert_eq(vim_diagnostic.severity, vim.diagnostic.severity.ERROR)
helpers.assert_eq(vim_diagnostic.user_data.target, "Missing")

local lsp_action = convert.to_lsp_code_action({
  title = "Add method descriptor",
  kind = "quickfix.at.addDescriptor",
  edits = {
    {
      documentUri = "file:///mod_at.cfg",
      edits = {
        {
          range = {
            start = { line = 0, character = 39 },
            ["end"] = { line = 0, character = 39 },
          },
          newText = "(Ljava/lang/String;FF)V",
        },
      },
    },
  },
  metadata = { member = "draw" },
})
helpers.assert_eq(lsp_action.title, "Add method descriptor")
helpers.assert_eq(lsp_action.kind, "quickfix.at.addDescriptor")
helpers.assert_eq(#lsp_action.edit.documentChanges, 1)
helpers.assert_eq(lsp_action.edit.documentChanges[1].edits[1].newText, "(Ljava/lang/String;FF)V")
helpers.assert_eq(lsp_action.data.member, "draw")

local result, unwrap_err = convert.unwrap_envelope({
  result = { diagnostics = { { code = "TEST" } } },
}, nil)
helpers.assert_nil(unwrap_err)
helpers.assert_eq(#result.diagnostics, 1)

local _, envelope_err = convert.unwrap_envelope({
  error = { message = "protocol mismatch, client=1 server=2" },
}, nil)
helpers.assert_eq(envelope_err, "protocol mismatch, client=1 server=2")

with_named_buffer("/project/src/main/java/com/example/mixin/ExampleMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local code_action_payload = protocol.build_code_action_payload(bufnr, {
    start = { line = 0, character = 7 },
    ["end"] = { line = 0, character = 19 },
  }, { "MIXIN_CLASS_NOT_LISTED_IN_CONFIG" })
  helpers.assert_eq(code_action_payload.diagnosticCodes[1], "MIXIN_CLASS_NOT_LISTED_IN_CONFIG")
  helpers.assert_eq(code_action_payload.range.start.line, 0)
  helpers.assert_eq(code_action_payload.context.languageId, "java")
end)

local nav_error = nil
vim.lsp.get_clients = function()
  return {}
end
navigation.definition(0, { 1, 1 }, function(_, err)
  nav_error = err
end)
helpers.assert_eq(nav_error, "mcdev: no active JDT LS client for this buffer")
vim.lsp.get_clients = original_get_clients

local hover_result = nil
protocol_module = package.loaded["mcdev.protocol"]
local original_hover = protocol_module.hover
protocol_module.hover = function(_, _, callback)
  callback({
    result = {
      contents = {
        "```mcdev\nclass com.example.target.SimpleTarget\n```",
      },
    },
  }, nil)
end
hover.hover(0, { 1, 1 }, function(result, err)
  helpers.assert_nil(err)
  hover_result = result
end)
helpers.assert_not_nil(hover_result)
helpers.assert_eq(hover_result.contents[1], "```mcdev\nclass com.example.target.SimpleTarget\n```")
protocol_module.hover = original_hover

with_named_buffer("/project/src/main/resources/mod.accesswidener", "accesswidener", {
  "accessWidener v2 named",
  "accessible class com/example/missing/Missing",
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_diagnostics = protocol_module.diagnostics
  protocol_module.diagnostics = function(_, _, callback)
    callback({
      result = {
        diagnostics = {
          {
            code = "AW_UNRESOLVED_CLASS",
            severity = "error",
            message = "Unresolved class",
            range = {
              start = { line = 1, character = 20 },
              ["end"] = { line = 1, character = 40 },
            },
          },
        },
      },
    }, nil)
  end
  local fetched_diagnostics = nil
  diagnostics.fetch(bufnr, { 2, 21 }, function(fetched, err)
    helpers.assert_nil(err)
    helpers.assert_eq(#fetched, 1)
    helpers.assert_eq(fetched[1].code, "AW_UNRESOLVED_CLASS")
    fetched_diagnostics = fetched
  end)
  helpers.assert_not_nil(fetched_diagnostics)
  diagnostics.refresh(bufnr)
  local published = vim.diagnostic.get(bufnr, { namespace = diagnostics.namespace })
  helpers.assert_eq(#published, 1)
  helpers.assert_eq(published[1].code, "AW_UNRESOLVED_CLASS")
  protocol_module.diagnostics = original_diagnostics
end)

local converted_actions = nil
local protocol_module = package.loaded["mcdev.protocol"]
local original_code_action = protocol_module.code_action
protocol_module.code_action = function(_, _, _, callback)
  callback({
    result = {
      actions = {
        {
          title = "Add method descriptor",
          kind = "quickfix.at.addDescriptor",
          edits = {
            {
              documentUri = "file:///mod_at.cfg",
              edits = {
                {
                  range = {
                    start = { line = 0, character = 39 },
                    ["end"] = { line = 0, character = 39 },
                  },
                  newText = "(Ljava/lang/String;FF)V",
                },
              },
            },
          },
        },
      },
    },
  }, nil)
end
code_action.code_actions(0, nil, { "AT_MISSING_METHOD_DESCRIPTOR" }, function(actions, err)
  helpers.assert_nil(err)
  converted_actions = actions
end)
helpers.assert_not_nil(converted_actions)
helpers.assert_eq(converted_actions[1].kind, "quickfix.at.addDescriptor")
protocol_module.code_action = original_code_action

helpers.assert_not_nil(mcdev.navigation)
helpers.assert_not_nil(mcdev.code_action)
helpers.assert_not_nil(mcdev.diagnostics)
helpers.assert_not_nil(mcdev.convert)
helpers.assert_not_nil(mcdev.lsp)
helpers.assert_true(#health.lines(0) > 0)

do
  local original_get_clients_for_health = vim.lsp.get_clients
  local original_notify_for_health = vim.notify
  local health_message = nil
  local commands_seen = {}
  local fake_client = {
    id = 99,
    name = "jdtls",
    config = { root_dir = vim.fn.getcwd() },
    request = function(method, params, callback)
      helpers.assert_eq(method, "workspace/executeCommand")
      commands_seen[params.command] = true
      if params.command == "mcdev.info" then
        callback(nil, {
          result = {
            lines = { "Extension loaded: true" },
            buildCommit = "test-commit",
            buildTime = "test-time",
            jarLocation = "file:///test.jar",
            registeredCommands = { "mcdev.info", "mcdev.completion" },
          },
        })
      elseif params.command == "mcdev.completion" then
        callback(nil, {
          result = {
            items = {
              { label = "tick(): void" },
            },
            debug = {
              zeroItemReason = vim.NIL,
              parseSource = "JDT_AST",
              parseConfidence = "HIGH",
              usedCompilationUnit = true,
              usedJavaProject = true,
              bindingResolvedCount = 1,
              bindingFailedCount = 0,
              semanticContextFound = true,
              fallbackAnnotationContextUsed = false,
              semanticTargetCount = 1,
              semanticMemberCount = 0,
              completionContextKind = "InjectMethod",
              warnings = {},
            },
          },
        })
      else
        callback(nil, { result = {} })
      end
    end,
  }
  vim.lsp.get_clients = function(opts)
    if opts and (opts.bufnr == 0 or opts.name == "jdtls") then
      return { fake_client }
    end
    return {}
  end
  vim.notify = function(message)
    health_message = message
  end
  health.health(0)
  helpers.assert_true(commands_seen["mcdev.info"])
  helpers.assert_true(commands_seen["mcdev.completion"])
  helpers.assert_true(commands_seen["mcdev.diagnostics"])
  helpers.assert_true(commands_seen["mcdev.hover"])
  helpers.assert_true(health_message:find("mcdev.info ping: OK", 1, true) ~= nil, health_message)
  helpers.assert_true(health_message:find("mcdev.completion itemCount: 1", 1, true) ~= nil, health_message)
  helpers.assert_true(health_message:find("usedCompilationUnit: true", 1, true) ~= nil, health_message)
  helpers.assert_true(health_message:find("fallbackAnnotationContextUsed: false", 1, true) ~= nil, health_message)
  vim.lsp.get_clients = original_get_clients_for_health
  vim.notify = original_notify_for_health
end

do
  local original_buf_request = vim.lsp.buf_request
  local original_definition = navigation.definition
  local fallback_used = false
  vim.lsp.buf_request = function(_, method, _, callback)
    helpers.assert_eq(method, "textDocument/definition")
    callback(nil, nil)
  end
  navigation.definition = function(_, _, callback)
    fallback_used = true
    callback({}, nil)
  end
  lsp_adapter.definition(0, { 1, 1 }, function(_, err)
    helpers.assert_nil(err)
  end)
  helpers.assert_true(fallback_used)
  vim.lsp.buf_request = original_buf_request
  navigation.definition = original_definition
end

print("mcdev-nvim adapter tests passed")
vim.cmd("qa!")
