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
local jdtls_helper = require("mcdev.jdtls")

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
helpers.assert_eq(payload.trigger.kind, "manual")
helpers.assert_eq(payload.options.preferredAtTarget, "smart")
helpers.assert_eq(payload.options.mixinClassInsert, "import")
helpers.assert_eq(payload.options.injectMethodDescriptor, "auto")
helpers.assert_not_nil(payload.context.client)
helpers.assert_eq(payload.context.client.name, "mcdev.nvim")

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
helpers.assert_eq(#blink_adapter:get_trigger_characters(), 3)

local completion_module = package.loaded["mcdev.completion"]
local original_complete = completion_module.complete
local blink_result = nil
completion_module.complete = function(callback)
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
blink_adapter:get_completions({}, function(result)
  blink_result = result
end)
helpers.assert_not_nil(blink_result)
helpers.assert_eq(blink_result.is_incomplete_forward, true)
helpers.assert_eq(#blink_result.items, 1)
helpers.assert_eq(blink_result.items[1].label, "tick(): void")
helpers.assert_eq(blink_result.items[1].insertText, "tick")

local cmp_result = nil
local cmp_source = cmp.source()
cmp_source:complete({}, function(items)
  cmp_result = items
end)
helpers.assert_not_nil(cmp_result)
helpers.assert_eq(#cmp_result, 1)
helpers.assert_eq(cmp_result[1].insertText, "tick")
completion_module.complete = original_complete

local commands = vim.api.nvim_get_commands({})
helpers.assert_not_nil(commands.McdevInfo)
helpers.assert_not_nil(commands.McdevReindex)

helpers.assert_eq(mcdev.options().insert.at_target, "smart")
helpers.assert_eq(mcdev.options().insert.mixin_class_import, true)
helpers.assert_eq(mcdev.options().insert.inject_method_descriptor, "auto")

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

print("mcdev-nvim adapter tests passed")
vim.cmd("qa!")
