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
local omnifunc = require("mcdev.omnifunc")
local stdio = require("mcdev.stdio")
local completion_transport = require("mcdev.transport")

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
config.options.jdtls.repo_root = mason_root .. "/not-a-repository"
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

local fake_repo = vim.fn.tempname()
local fake_built_jar = fake_repo .. "/mcdev-jdtls-extension/build/libs/io.github.mcdev.jdtls-9.8.7.jar"
vim.fn.mkdir(vim.fn.fnamemodify(fake_built_jar, ":h"), "p")
vim.fn.writefile({}, fake_built_jar)
local configured_jar = config.options.jdtls.extension_jar
config.options.jdtls.extension_jar = nil
helpers.assert_eq(
  vim.fs.normalize(jdtls_helper.resolve_extension_jar({ repo_root = fake_repo, mason = { enabled = false } })),
  vim.fs.normalize(fake_built_jar)
)
config.options.jdtls.extension_jar = configured_jar

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

local attribute_item = completion.to_lsp_item({
  label = 'method = "…"',
  filterText = "method",
  insertText = 'method = "${1}"$0',
  insertTextFormat = "snippet",
  kind = "keyword",
  sortKey = "0000_method",
  additionalEdits = {},
  metadata = { source = "mixin.attribute" },
})
helpers.assert_eq(attribute_item.insertTextFormat, vim.lsp.protocol.InsertTextFormat.Snippet)
helpers.assert_eq(attribute_item.preselect, true)

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
        kind = vim.lsp.protocol.CompletionItemKind.Value,
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
vim.bo[0].filetype = "java"
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

completion_module.complete = function(callback, _, _, opts)
  adapter_sources[#adapter_sources + 1] = opts and opts.source or nil
  callback({
    isIncomplete = true,
    items = {
      completion.to_lsp_item({
        label = 'method = "…"',
        filterText = "method",
        insertText = 'method = "${1}"$0',
        insertTextFormat = "snippet",
        kind = "keyword",
        sortKey = "0000_method",
        edit = nil,
        additionalEdits = {},
        metadata = { source = "mixin.attribute" },
      }),
    },
  })
end
vim.api.nvim_buf_set_lines(0, 0, -1, false, { "@Inject(meth" })
local attribute_blink_result = nil
blink_adapter:get_completions({ bufnr = 0, cursor = { 1, #"@Inject(meth" } }, function(result)
  attribute_blink_result = result
end)
helpers.assert_eq(attribute_blink_result.items[1].score_offset, 100)
helpers.assert_eq(attribute_blink_result.items[1].insertTextFormat, vim.lsp.protocol.InsertTextFormat.Snippet)

local cmp_result = nil
local cmp_source = cmp.source()
cmp_source:complete({}, function(items)
  cmp_result = items
end)
helpers.assert_not_nil(cmp_result)
helpers.assert_eq(#cmp_result.items, 1)
helpers.assert_eq(cmp_result.items[1].insertText, 'method = "${1}"$0')
helpers.assert_eq(cmp_result.items[1].insertTextFormat, vim.lsp.protocol.InsertTextFormat.Snippet)
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
helpers.assert_eq(mcdev.options().completion.omnifunc, false)
helpers.assert_eq(mcdev.options().completion.omnifunc_timeout_ms, 500)
helpers.assert_eq(mcdev.options().diagnostics.enabled, false)
helpers.assert_eq(mcdev.options().diagnostics.events[1], "BufWritePost")
helpers.assert_eq(diagnostics.running, false)
helpers.assert_eq(mcdev.options().standard_lsp.prefer, true)

do
  local original_complete_for_omnifunc = completion.complete
  local original_timeout = config.options.completion.omnifunc_timeout_ms
  config.options.completion.omnifunc_timeout_ms = 10
  completion.complete = function() end
  local items = omnifunc.complete(0, "")
  helpers.assert_eq(#items, 0)
  helpers.assert_true(omnifunc.last_timeout)
  completion.complete = original_complete_for_omnifunc
  config.options.completion.omnifunc_timeout_ms = original_timeout
end

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

local function draw_completion_response()
  return {
    result = {
      items = {
        {
          label = "draw(String): void",
          insertText = "draw",
          kind = "value",
          sortKey = "0200_draw",
          filterText = "draw",
          detail = "SimpleTarget",
          additionalEdits = {},
          metadata = { source = "mixin.injectMethod" },
        },
      },
    },
  }
end

local function mock_completion_transport()
  return function()
  end
end

local function install_completion_transport_mock(on_request)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_completion = protocol_module.completion
  protocol_module.completion = function(callback, bufnr, position)
    return on_request(callback, bufnr, position)
  end
  return original_completion
end

local function install_stdio_transport_mock(on_request)
  local original_request = stdio.request
  stdio.request = function(payload, callback, opts)
    return on_request(payload, callback, opts)
  end
  return original_request
end

local function install_tracked_completion_client(bufnr, root_dir)
  local original_get_clients = vim.lsp.get_clients
  local next_request_id = 0
  local pending_by_id = {}
  local cancel_calls = {}
  local fake_client = {
    name = "jdtls",
    config = { root_dir = root_dir or "/project" },
    request = function(self, _, params, handler, request_bufnr)
      next_request_id = next_request_id + 1
      pending_by_id[next_request_id] = {
        handler = handler,
        bufnr = request_bufnr,
      }
      return true, next_request_id
    end,
    cancel_request = function(self, id)
      cancel_calls[#cancel_calls + 1] = id
    end,
  }

  vim.lsp.get_clients = function(opts)
    if opts and opts.bufnr and opts.bufnr ~= bufnr then
      return {}
    end
    if opts and opts.name == "jdtls" then
      return { fake_client }
    end
    if opts and opts.bufnr == bufnr then
      return { fake_client }
    end
    return { fake_client }
  end

  return {
    fake_client = fake_client,
    pending_by_id = pending_by_id,
    cancel_calls = cancel_calls,
    restore = function()
      vim.lsp.get_clients = original_get_clients
    end,
    resolve = function(index, response)
      local entry = pending_by_id[index]
      if entry then
        entry.handler(nil, response)
      end
    end,
    pending_count = function()
      local count = 0
      for _ in pairs(pending_by_id) do
        count = count + 1
      end
      return count
    end,
  }
end

local function assert_overlapping_prefix_lifecycle(bufnr, resolve_order)
  local transport = install_tracked_completion_client(bufnr)

  local callbacks = {}
  local cancelled_before = completion.cancelled_count

  completion.complete(function(result)
    callbacks[#callbacks + 1] = result
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "lifecycle" })
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dr' })
  completion.complete(function(result)
    callbacks[#callbacks + 1] = result
  end, bufnr, { 1, #'@Inject(method = "dr' }, { source = "lifecycle" })
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dra' })
  completion.complete(function(result)
    callbacks[#callbacks + 1] = result
  end, bufnr, { 1, #'@Inject(method = "dra' }, { source = "lifecycle" })

  helpers.assert_eq(transport.pending_count(), 3)
  helpers.assert_eq(#transport.cancel_calls, 2)
  helpers.assert_eq(transport.cancel_calls[1], 1)
  helpers.assert_eq(transport.cancel_calls[2], 2)
  helpers.assert_eq(completion.cancelled_count - cancelled_before, 2)

  local response = draw_completion_response()
  for _, index in ipairs(resolve_order) do
    transport.resolve(index, response)
  end

  helpers.assert_eq(#callbacks, 1)
  helpers.assert_eq(#callbacks[1].items, 1)
  helpers.assert_eq(callbacks[1].items[1].insertText, "draw")

  transport.restore()
end

with_named_buffer("/project/src/main/java/com/example/mixin/ExampleMixin.java", "java", {
  '@Inject(method = "dr',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local server_requests = 0
  local original_completion = install_completion_transport_mock(function(callback, _, position)
    server_requests = server_requests + 1
    local prefix_end = position[2]
    callback({
      result = {
        items = {
          {
            label = "draw(String): void",
            insertText = "draw",
            kind = "value",
            sortKey = "0200_draw",
            filterText = "draw",
            detail = "SimpleTarget",
            additionalEdits = {},
            edit = {
              range = {
                start = { line = 0, character = #'@Inject(method = "' },
                ["end"] = { line = 0, character = prefix_end },
              },
              newText = "draw",
            },
            metadata = { source = "mixin.injectMethod" },
          },
        },
        debug = {},
      },
    }, nil)
    return mock_completion_transport()
  end)

  local first = nil
  completion.complete(function(result)
    first = result
  end, bufnr, { 1, #'@Inject(method = "dr' }, { source = "manual" })
  helpers.assert_eq(server_requests, 1)
  helpers.assert_eq(#first.items, 1)

  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dra' })
  local second = nil
  completion.complete(function(result)
    second = result
  end, bufnr, { 1, #'@Inject(method = "dra' }, { source = "manual" })
  helpers.assert_not_nil(second)
  helpers.assert_eq(server_requests, 2)
  helpers.assert_eq(#second.items, 1)
  helpers.assert_eq(second.items[1].insertText, "draw")
  helpers.assert_eq(second.items[1].textEdit.range["end"].character, #'@Inject(method = "dra')
  vim.lsp.util.apply_text_edits({ second.items[1].textEdit }, bufnr, "utf-8")
  helpers.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], '@Inject(method = "draw')

  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/LimitedPageMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local server_requests = 0
  local original_completion = install_completion_transport_mock(function(callback, _, position)
    server_requests = server_requests + 1
    if server_requests == 1 then
      local items = {}
      for index = 1, 50 do
        items[#items + 1] = {
          label = string.format("candidate%02d(): void", index),
          insertText = string.format("candidate%02d", index),
          kind = "value",
          sortKey = string.format("0200_candidate%02d", index),
          filterText = string.format("candidate%02d", index),
          detail = "SimpleTarget",
          additionalEdits = {},
          metadata = { source = "mixin.injectMethod" },
        }
      end
      callback({ result = { items = items } }, nil)
      return mock_completion_transport()
    end
    local prefix_end = position[2]
    callback({
      result = {
        items = {
          {
            label = "zzz(): void",
            insertText = "zzz",
            kind = "value",
            sortKey = "0200_zzz",
            filterText = "zzz",
            detail = "SimpleTarget",
            additionalEdits = {},
            edit = {
              range = {
                start = { line = 0, character = #'@Inject(method = "' },
                ["end"] = { line = 0, character = prefix_end },
              },
              newText = "zzz",
            },
            metadata = { source = "mixin.injectMethod" },
          },
        },
      },
    }, nil)
    return mock_completion_transport()
  end)

  local first = nil
  completion.complete(function(result)
    first = result
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "limited-page" })
  helpers.assert_eq(server_requests, 1)
  helpers.assert_eq(#first.items, 50)
  helpers.assert_nil(vim.tbl_filter(function(item)
    return item.insertText == "zzz"
  end, first.items)[1])

  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "zzz' })
  local second = nil
  completion.complete(function(result)
    second = result
  end, bufnr, { 1, #'@Inject(method = "zzz' }, { source = "limited-page" })
  helpers.assert_eq(server_requests, 2)
  helpers.assert_not_nil(second)
  helpers.assert_eq(#second.items, 1)
  helpers.assert_eq(second.items[1].insertText, "zzz")

  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ErrorMixin.java", "java", {
  '@Inject(method = "error',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_completion = install_completion_transport_mock(function(callback)
    callback({ error = { message = "completion failed" } }, nil)
    return mock_completion_transport()
  end)
  local failed = nil
  completion.complete(function(result)
    failed = result
  end, bufnr, { 1, #'@Inject(method = "error' }, { source = "error-test" })
  helpers.assert_not_nil(failed)
  helpers.assert_eq(#failed.items, 0)
  helpers.assert_eq(completion.last_error, "completion failed")
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/RequestStartFailureMixin.java", "java", {
  '@Inject(method = "error',
}, function(bufnr)
  local original_get_clients = vim.lsp.get_clients
  local request_count = 0
  local pending = nil
  local fake_client = {
    name = "jdtls",
    config = { root_dir = "/project" },
    request = function(self, _, _, handler)
      request_count = request_count + 1
      if request_count == 1 then
        return false, nil
      end
      pending = handler
      return true, request_count
    end,
    cancel_request = function() end,
  }
  vim.lsp.get_clients = function()
    return { fake_client }
  end

  local failed = nil
  completion.complete(function(result)
    failed = result
  end, bufnr, { 1, #'@Inject(method = "error' }, { source = "request-start-failure" })
  helpers.assert_not_nil(failed)
  helpers.assert_true(failed.isIncomplete)
  helpers.assert_eq(#failed.items, 0)
  helpers.assert_eq(completion.last_error, "mcdev: failed to start JDT LS request")

  local recovered = nil
  completion.complete(function(result)
    recovered = result
  end, bufnr, { 1, #'@Inject(method = "error' }, { source = "request-start-failure" })
  helpers.assert_eq(request_count, 2)
  pending(nil, draw_completion_response())
  helpers.assert_not_nil(recovered)
  helpers.assert_eq(#recovered.items, 1)
  helpers.assert_eq(recovered.items[1].insertText, "draw")

  vim.lsp.get_clients = original_get_clients
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ConcurrentMixin.java", "java", {
  '@WrapOperation(method = "")',
}, function(bufnr)
  local transport = install_tracked_completion_client(bufnr)

  local first = nil
  local second = nil
  local cancelled_before = completion.cancelled_count
  completion.complete(function(result)
    first = result
  end, bufnr, { 1, #'@WrapOperation(method = "' }, { source = "blink-concurrent" })
  completion.complete(function(result)
    second = result
  end, bufnr, { 1, #'@WrapOperation(method = "' }, { source = "blink-concurrent" })
  helpers.assert_eq(transport.pending_count(), 2)
  helpers.assert_eq(#transport.cancel_calls, 1)
  helpers.assert_eq(transport.cancel_calls[1], 1)
  helpers.assert_eq(completion.cancelled_count - cancelled_before, 1)

  local response = {
    result = {
      items = {
        {
          label = "tick(MovementContext): void",
          insertText = "tick",
          kind = "value",
          sortKey = "0200_tick",
          metadata = { source = "mixinextras.injectMethod" },
        },
      },
    },
  }
  transport.resolve(1, response)
  helpers.assert_nil(first)
  transport.resolve(2, response)
  helpers.assert_not_nil(second)
  helpers.assert_eq(#second.items, 1)
  helpers.assert_eq(second.items[1].insertText, "tick")

  transport.restore()
end)

with_named_buffer("/project/src/main/java/com/example/mixin/LifecycleNewestFirstMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  assert_overlapping_prefix_lifecycle(bufnr, { 3, 2, 1 })
end)

with_named_buffer("/project/src/main/java/com/example/mixin/LifecycleOldestFirstMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  assert_overlapping_prefix_lifecycle(bufnr, { 1, 2, 3 })
end)

with_named_buffer("/project/src/main/java/com/example/mixin/RapidTypingMixin.java", "java", {
  '@WrapOperation(method = "destro")',
}, function(bufnr)
  vim.api.nvim_set_current_buf(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local pending = {}
  local original_completion = install_completion_transport_mock(function(callback)
    pending[#pending + 1] = callback
    return mock_completion_transport()
  end)

  local destroy_block_item = function(prefix_end)
    return {
      label = "destroyBlock(MovementContext): void",
      insertText = "destroyBlock",
      kind = "value",
      sortKey = "0200_destroyBlock",
      filterText = "destroyBlock",
      edit = {
        range = {
          start = { line = 0, character = #'@WrapOperation(method = "' },
          ["end"] = { line = 0, character = prefix_end },
        },
        newText = "destroyBlock",
      },
      metadata = { source = "mixinextras.injectMethod" },
    }
  end

  local stale_dropped_before = completion.stale_dropped_count
  local stale = nil
  local queued = nil
  local rapid_blink = blink.source()
  rapid_blink:get_completions({ bufnr = bufnr, cursor = { 1, #'@WrapOperation(method = "destro' } }, function(result)
    stale = result
    rapid_blink:get_completions({ bufnr = bufnr, cursor = { 1, #'@WrapOperation(method = "destroy' } }, function(result)
      queued = result
    end)
  end)
  helpers.assert_eq(#pending, 1)

  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@WrapOperation(method = "destroy")' })
  vim.api.nvim_win_set_cursor(0, { 1, #'@WrapOperation(method = "destroy' })
  pending[1]({
    result = {
      items = {
        destroy_block_item(#'@WrapOperation(method = "destro'),
      },
    },
  }, nil)
  helpers.assert_not_nil(stale)
  helpers.assert_true(stale.is_incomplete_forward)
  helpers.assert_true(stale.is_incomplete_backward)
  helpers.assert_eq(#stale.items, 0)
  helpers.assert_eq(completion.stale_dropped_count - stale_dropped_before, 1)
  helpers.assert_eq(#pending, 2)
  pending[2]({
    result = {
      items = {
        destroy_block_item(#'@WrapOperation(method = "destroy'),
      },
    },
  }, nil)
  helpers.assert_not_nil(queued)
  helpers.assert_eq(#queued.items, 1)
  helpers.assert_eq(queued.items[1].insertText, "destroyBlock")
  vim.lsp.util.apply_text_edits({ queued.items[1].textEdit }, bufnr, "utf-8")
  helpers.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], '@WrapOperation(method = "destroyBlock")')

  local extended = nil
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@WrapOperation(method = "destroyB")' })
  vim.api.nvim_win_set_cursor(0, { 1, #'@WrapOperation(method = "destroyB' })
  rapid_blink:get_completions({ bufnr = bufnr, cursor = { 1, #'@WrapOperation(method = "destroyB' } }, function(result)
    extended = result
  end)
  helpers.assert_eq(#pending, 3)
  pending[3]({
    result = {
      items = {
        destroy_block_item(#'@WrapOperation(method = "destroyB'),
      },
    },
  }, nil)
  helpers.assert_not_nil(extended)
  helpers.assert_eq(#extended.items, 1)
  helpers.assert_eq(extended.items[1].cursor_column, #'@WrapOperation(method = "destroyB')
  helpers.assert_eq(
    extended.items[1].textEdit.range["end"].character,
    #'@WrapOperation(method = "destroyB'
  )

  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/QueuedCompletionMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local pending = {}
  local original_completion = install_completion_transport_mock(function(callback)
    pending[#pending + 1] = callback
    return mock_completion_transport()
  end)

  -- A queued Blink request keeps the active operation alive while the buffer changes.
  local stale = nil
  completion.complete(function(result)
    stale = result
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "blink-queue" })
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dr' })
  pending[1]({ error = { message = "completion failed after edit" } }, nil)
  helpers.assert_not_nil(stale)
  helpers.assert_true(stale.isStale)
  helpers.assert_nil(stale.isIncomplete)
  helpers.assert_nil(stale.items)

  local cmp_stale_callback_calls = 0
  local cmp_stale_result = nil
  cmp.source():complete({
    context = { bufnr = bufnr, cursor = { 1, #'@Inject(method = "dr' } },
  }, function(result)
    cmp_stale_callback_calls = cmp_stale_callback_calls + 1
    cmp_stale_result = result
  end)
  helpers.assert_eq(#pending, 2)
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { '@Inject(method = "dra' })
  pending[2]({ error = { message = "completion failed after edit" } }, nil)
  helpers.assert_eq(cmp_stale_callback_calls, 1)
  helpers.assert_not_nil(cmp_stale_result)
  helpers.assert_true(cmp_stale_result.isIncomplete)
  helpers.assert_eq(#cmp_stale_result.items, 0)

  -- cmp can issue a fresh request after the stale result settled.
  local cmp_fresh = nil
  cmp.source():complete({
    context = { bufnr = bufnr, cursor = { 1, #'@Inject(method = "dra' } },
  }, function(result)
    cmp_fresh = result
  end)
  helpers.assert_eq(#pending, 3)
  pending[3](draw_completion_response(), nil)
  helpers.assert_not_nil(cmp_fresh)
  helpers.assert_eq(#cmp_fresh.items, 1)

  -- The underlying request lifecycle also recovers for the next Blink request.
  local fresh = nil
  completion.complete(function(result)
    fresh = result
  end, bufnr, { 1, #'@Inject(method = "dra' }, { source = "blink-queue" })
  helpers.assert_eq(#pending, 4)
  pending[4](draw_completion_response(), nil)
  helpers.assert_not_nil(fresh)
  helpers.assert_eq(#fresh.items, 1)

  -- Explicitly cancel the currently active request; its late response is silent.
  local cancelled = nil
  local cancel = completion.complete(function(result)
    cancelled = result
  end, bufnr, { 1, #'@Inject(method = "dr' }, { source = "blink-queue" })
  helpers.assert_eq(#pending, 5)
  cancel()
  pending[5]({ result = { items = {} } }, nil)
  helpers.assert_nil(cancelled)

  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ProvisionalCompletionMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local jdt_callbacks = {}
  local helper_callbacks = {}
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callbacks[#jdt_callbacks + 1] = callback
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(payload, callback)
    helpers.assert_not_nil(payload.context)
    helpers.assert_not_nil(payload.context.bufferTextFallback)
    helper_callbacks[#helper_callbacks + 1] = callback
    return function() end
  end)

  local jdt_response = draw_completion_response()
  jdt_response.result.items[1].detail = "JDT rich metadata"
  jdt_response.result.items[1].metadata = { source = "jdt.mixin.injectMethod", richer = true }
  jdt_response.result.items[#jdt_response.result.items + 1] = {
    label = "tick(MovementContext): void",
    insertText = "tick",
    kind = "value",
    sortKey = "0200_tick",
    detail = "SimpleTarget",
    metadata = { source = "mixin.injectMethod" },
  }

  local blink_results = {}
  blink.source():get_completions({
    bufnr = bufnr,
    cursor = { 1, #'@Inject(method = "d' },
  }, function(result)
    blink_results[#blink_results + 1] = result
  end)
  helpers.assert_eq(#helper_callbacks, 1)
  helpers.assert_eq(#jdt_callbacks, 1)
  helpers.assert_eq(#blink_results, 0)

  helper_callbacks[1](draw_completion_response(), nil)
  helpers.assert_eq(#blink_results, 1)
  helpers.assert_true(blink_results[1].is_incomplete_forward)
  helpers.assert_eq(#blink_results[1].items, 1)
  helpers.assert_eq(blink_results[1].items[1].insertText, "draw")

  jdt_callbacks[1](jdt_response, nil)
  helpers.assert_eq(#blink_results, 2)
  helpers.assert_eq(blink_results[2].is_incomplete_forward, false)
  helpers.assert_eq(#blink_results[2].items, 1)
  helpers.assert_eq(blink_results[2].items[1].insertText, "tick")

  local cmp_results = {}
  cmp.source():complete({
    context = { bufnr = bufnr, cursor = { 1, #'@Inject(method = "d' } },
  }, function(result)
    cmp_results[#cmp_results + 1] = result
  end)
  helpers.assert_eq(#helper_callbacks, 2)
  helpers.assert_eq(#jdt_callbacks, 2)
  helper_callbacks[2](draw_completion_response(), nil)
  helpers.assert_eq(#cmp_results, 1)
  helpers.assert_eq(#cmp_results[1].items, 1)
  jdt_callbacks[2](jdt_response, nil)
  helpers.assert_eq(#cmp_results, 2)
  helpers.assert_eq(#cmp_results[2].items, 2)
  helpers.assert_eq(cmp_results[2].items[1].insertText, "draw")
  helpers.assert_eq(cmp_results[2].items[1].detail, "JDT rich metadata")
  helpers.assert_true(cmp_results[2].items[1].data.richer)
  helpers.assert_eq(cmp_results[2].items[2].insertText, "tick")

  local manual_results = {}
  completion.complete(function(result)
    manual_results[#manual_results + 1] = result
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "manual" })
  helpers.assert_eq(#helper_callbacks, 3)
  helpers.assert_eq(#jdt_callbacks, 3)
  helper_callbacks[3](draw_completion_response(), nil)
  helpers.assert_eq(#manual_results, 0)
  jdt_callbacks[3](jdt_response, nil)
  helpers.assert_eq(#manual_results, 1)
  helpers.assert_eq(#manual_results[1].items, 2)

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/TransportFallbackMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_completion = install_completion_transport_mock(function(callback)
    callback(draw_completion_response(), nil)
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(_, callback)
    callback({ result = { items = {} } }, nil)
    return function() end
  end)
  local original_transport_request = completion_transport.request
  local transport_calls = 0
  completion_transport.request = function(_, callback)
    transport_calls = transport_calls + 1
    callback({ error = { code = "INCOMPLETE_PROJECT_CONTEXT", message = "JDT project is not ready" } }, nil)
    return function() end
  end

  local result = nil
  completion.complete(function(value)
    result = value
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "transport-fallback" })
  helpers.assert_eq(transport_calls, 1)
  helpers.assert_not_nil(result)
  helpers.assert_eq(#result.items, 1)
  helpers.assert_eq(result.items[1].insertText, "draw")

  completion_transport.request = original_transport_request
  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

for _, unavailable_state in ipairs({ "endpoint", "classpath" }) do
with_named_buffer("/project/src/main/java/com/example/mixin/TransportReadyMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local jdt_callback, ready_callback
  local jdt_cancelled = false
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callback = callback
    return function() jdt_cancelled = true end
  end)
  local original_stdio = install_stdio_transport_mock(function() return function() end end)
  local original_request, original_ready = completion_transport.request, completion_transport.when_ready
  completion_transport.request = function(_, callback)
    if unavailable_state == "endpoint" then return nil, "unavailable" end
    callback({ error = { code = "INCOMPLETE_PROJECT_CONTEXT", message = "dependencies not ready" } }, nil)
    return function() end
  end
  completion_transport.when_ready = function(_, callback)
    ready_callback = callback
    return function() end
  end
  local results = {}
  completion.complete(function(result) results[#results + 1] = result end, bufnr, { 1, #'@Inject(method = "d' })
  helpers.assert_not_nil(jdt_callback, "unavailable endpoint must retain normal fallback")
  helpers.assert_eq(#results, 0)
  completion_transport.request = function(_, callback)
    callback(draw_completion_response(), nil)
    return function() end
  end
  ready_callback()
  helpers.assert_eq(#results, 1, "ready endpoint must finish without waiting for JDT index gate")
  helpers.assert_true(jdt_cancelled)
  jdt_callback(draw_completion_response(), nil)
  helpers.assert_eq(#results, 1, "cancelled fallback must not publish again")
  completion_transport.request, completion_transport.when_ready = original_request, original_ready
  stdio.request = original_stdio
  protocol.completion = original_completion
end)
end

with_named_buffer("/project/src/main/java/com/example/mixin/ProvisionalFailureMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local jdt_callback = nil
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callback = callback
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function()
    return nil, "mcdev: helper unavailable"
  end)

  local results = {}
  blink.source():get_completions({
    bufnr = bufnr,
    cursor = { 1, #'@Inject(method = "d' },
  }, function(result)
    results[#results + 1] = result
  end)
  helpers.assert_eq(#results, 0)
  jdt_callback({ error = { message = "jdt failed" } }, nil)
  helpers.assert_eq(#results, 1)
  helpers.assert_true(results[1].is_incomplete_forward)
  helpers.assert_eq(#results[1].items, 0)

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ProvisionalJdtFailureMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local helper_callback = nil
  local jdt_callback = nil
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callback = callback
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(_, callback)
    helper_callback = callback
    return function() end
  end)

  local results = {}
  blink.source():get_completions({
    bufnr = bufnr,
    cursor = { 1, #'@Inject(method = "d' },
  }, function(result)
    results[#results + 1] = result
  end)
  helper_callback(draw_completion_response(), nil)
  helpers.assert_eq(#results, 1)
  jdt_callback({ error = { message = "jdt failed after provisional" } }, nil)
  helpers.assert_eq(#results, 2)
  helpers.assert_true(results[2].is_incomplete_forward)
  helpers.assert_eq(#results[2].items, 0)
  helpers.assert_eq(#results[1].items, 1)

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/JdtFailureBeforeHelperMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local helper_callback = nil
  local jdt_callback = nil
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callback = callback
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(_, callback)
    helper_callback = callback
    return function() end
  end)

  local results = {}
  blink.source():get_completions({
    bufnr = bufnr,
    cursor = { 1, #'@Inject(method = "d' },
  }, function(result)
    results[#results + 1] = result
  end)
  jdt_callback({ error = { message = "jdt failed before helper" } }, nil)
  helpers.assert_eq(#results, 0)
  helper_callback(draw_completion_response(), nil)
  helpers.assert_eq(#results, 2)
  helpers.assert_eq(#results[1].items, 1)
  helpers.assert_eq(results[1].items[1].insertText, "draw")
  helpers.assert_eq(#results[2].items, 0)

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/MalformedHelperMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local jdt_callback = nil
  local original_completion = install_completion_transport_mock(function(callback)
    jdt_callback = callback
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(_, callback)
    callback({ result = { items = "malformed" } }, nil)
    return function() end
  end)

  local result = nil
  completion.complete(function(value)
    result = value
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "malformed-helper" })
  helpers.assert_not_nil(jdt_callback)
  jdt_callback(draw_completion_response(), nil)
  helpers.assert_not_nil(result)
  helpers.assert_eq(#result.items, 1)
  helpers.assert_eq(result.items[1].insertText, "draw")

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ReentrantCompletionMixin.java", "java", {
  '@Inject(method = "d',
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local jdt_requests = 0
  local original_completion = install_completion_transport_mock(function()
    jdt_requests = jdt_requests + 1
    return function() end
  end)
  local original_stdio = install_stdio_transport_mock(function(_, callback)
    callback(draw_completion_response(), nil)
    return function() end
  end)

  local nested = true
  completion.complete(function()
    if nested then
      nested = false
      local cancel = completion.complete(function() end, bufnr, { 1, #'@Inject(method = "d' }, {
        source = "reentrant-inner",
      })
      cancel()
    end
  end, bufnr, { 1, #'@Inject(method = "d' }, { source = "reentrant-outer", stream = true })
  helpers.assert_eq(jdt_requests, 1)

  stdio.request = original_stdio
  protocol_module.completion = original_completion
end)

with_named_buffer("/project/src/main/java/com/example/mixin/ChangedTargetMixin.java", "java", {
  "@Mixin(FirstTarget.class)",
  '@WrapOperation(method = "destro")',
}, function(bufnr)
  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_win_set_cursor(0, { 2, #'@WrapOperation(method = "destro' })
  local protocol_module = package.loaded["mcdev.protocol"]
  local pending = nil
  local original_completion = install_completion_transport_mock(function(callback)
    pending = callback
    return mock_completion_transport()
  end)

  local stale_dropped_before = completion.stale_dropped_count
  local stale_callback_calls = 0
  blink.source():get_completions({ bufnr = bufnr, cursor = { 2, #'@WrapOperation(method = "destro' } }, function()
    stale_callback_calls = stale_callback_calls + 1
  end)
  helpers.assert_not_nil(pending)

  vim.api.nvim_buf_set_lines(bufnr, 0, 2, false, {
    "@Mixin(SecondTarget.class)",
    '@WrapOperation(method = "destroy")',
  })
  vim.api.nvim_win_set_cursor(0, { 2, #'@WrapOperation(method = "destroy' })
  pending({
    result = {
      items = {
        {
          label = "destroyBlock(MovementContext): void",
          insertText = "destroyBlock",
          kind = "value",
          filterText = "destroyBlock",
          metadata = { source = "mixinextras.injectMethod" },
        },
      },
    },
  }, nil)
  helpers.assert_eq(stale_callback_calls, 1)
  helpers.assert_eq(completion.stale_dropped_count - stale_dropped_before, 1)

  protocol_module.completion = original_completion
end)

do
  local original_request = protocol.request
  local original_notify = vim.notify
  local message = nil
  protocol.request = function(_, _, callback)
    callback({ error = { message = "reindex failed" } }, nil)
  end
  vim.notify = function(value)
    message = value
  end
  protocol.reindex()
  helpers.assert_eq(message, "mcdev: reindex failed")
  protocol.request = original_request
  vim.notify = original_notify
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
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  local payload = protocol.build_completion_payload(bufnr, { 1, 8 })
  helpers.assert_eq(payload.context.languageId, "java")
  require("mcdev.attach").setup(bufnr)
  helpers.assert_eq(vim.bo[bufnr].omnifunc, "")
end)

with_named_buffer("/project/src/main/java/com/example/mixin/OmnifuncMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local original_completion_options = vim.deepcopy(config.options.completion)
  config.options.completion.omnifunc = true
  require("mcdev.attach").setup(bufnr)
  helpers.assert_eq(vim.bo[bufnr].omnifunc, "v:lua.require'mcdev.omnifunc'.complete")
  config.options.completion = original_completion_options
end)

with_named_buffer("/project/src/main/java/com/example/PlainService.java", "java", {
  "public final class PlainService {}",
}, function(bufnr)
  helpers.assert_true(buffer.is_mcdev_buffer(bufnr))
  helpers.assert_eq(buffer.is_mcdev_completion_context(bufnr), false)
  helpers.assert_eq(blink_adapter:enabled({ bufnr = bufnr }), false)

  local plain_java_requests = 0
  local original_complete = completion_module.complete
  completion_module.complete = function(callback)
    plain_java_requests = plain_java_requests + 1
    callback({ items = {} })
  end
  local plain_java_result = nil
  blink_adapter:get_completions({ bufnr = bufnr, cursor = { 1, 1 } }, function(result)
    plain_java_result = result
  end)
  helpers.assert_eq(plain_java_requests, 0)
  helpers.assert_not_nil(plain_java_result)
  helpers.assert_eq(#plain_java_result.items, 0)
  completion_module.complete = original_complete
end)

with_named_buffer("/project/src/main/resources/data.json", "json", {
  '{"name":"plain"}',
}, function(bufnr)
  helpers.assert_eq(buffer.is_mcdev_completion_context(bufnr), false)
  helpers.assert_eq(blink_adapter:enabled({ bufnr = bufnr }), false)

  local plain_json_requests = 0
  local original_complete = completion_module.complete
  completion_module.complete = function(callback)
    plain_json_requests = plain_json_requests + 1
    callback({ items = {} })
  end
  local plain_json_result = nil
  blink_adapter:get_completions({ bufnr = bufnr, cursor = { 1, 2 } }, function(result)
    plain_json_result = result
  end)
  helpers.assert_eq(plain_json_requests, 0)
  helpers.assert_not_nil(plain_json_result)
  helpers.assert_eq(#plain_json_result.items, 0)
  completion_module.complete = original_complete
end)

with_named_buffer("/project/src/main/java/com/example/mixin/InjectStringMixin.java", "java", {
  '@Inject(method = "dr',
}, function(bufnr)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_eq(blink_adapter:enabled({ bufnr = bufnr }), true)

  local mixin_string_requests = 0
  local original_complete = completion_module.complete
  completion_module.complete = function(callback)
    mixin_string_requests = mixin_string_requests + 1
    callback({ items = {} })
  end
  blink_adapter:get_completions({ bufnr = bufnr, cursor = { 1, #'@Inject(method = "dr' } }, function() end)
  helpers.assert_eq(mixin_string_requests, 1)
  completion_module.complete = original_complete
end)

with_named_buffer("/project/src/main/java/com/example/mixin/BlinkCancelMixin.java", "java", {
  '@Inject(method = "dr',
}, function(bufnr)
  local cancel_calls = 0
  local menu_callback_calls = 0
  local original_complete = completion_module.complete
  completion_module.complete = function(_, _, _, opts)
    helpers.assert_eq(opts and opts.source, "blink")
    local cancelled = false
    return function()
      if cancelled then
        return
      end
      cancelled = true
      cancel_calls = cancel_calls + 1
    end
  end

  local cancel = blink_adapter:get_completions({ bufnr = bufnr, cursor = { 1, #'@Inject(method = "dr' } }, function()
    menu_callback_calls = menu_callback_calls + 1
  end)
  helpers.assert_eq(type(cancel), "function")
  helpers.assert_eq(menu_callback_calls, 0)
  cancel()
  cancel()
  helpers.assert_eq(cancel_calls, 1)

  completion_module.complete = original_complete
end)

with_named_buffer("/project/src/main/resources/example.mixins.json", "json", {
  '{"package":"com.example.mixin","mixins":[]}',
}, function(bufnr)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_eq(blink_adapter:enabled({ bufnr = bufnr }), true)

  local mixin_json_requests = 0
  local original_complete = completion_module.complete
  completion_module.complete = function(callback)
    mixin_json_requests = mixin_json_requests + 1
    callback({ items = {} })
  end
  blink_adapter:get_completions({ bufnr = bufnr, cursor = { 1, 12 } }, function() end)
  helpers.assert_eq(mixin_json_requests, 1)
  completion_module.complete = original_complete
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
    request = function(self, method, params, handler, request_bufnr)
      requested_command = params.command
      requested_bufnr = request_bufnr
      handler(nil, { result = { items = {} } })
      return true, 1
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

with_named_buffer("/project/src/main/java/com/example/mixin/QueuedDiagnosticsMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local protocol_module = package.loaded["mcdev.protocol"]
  local original_diagnostics = protocol_module.diagnostics
  local callbacks = {}
  protocol_module.diagnostics = function(_, _, callback)
    callbacks[#callbacks + 1] = callback
  end

  diagnostics.refresh(bufnr, { in_flight_policy = "latest", stale_result_policy = "drop" })
  vim.api.nvim_buf_set_lines(bufnr, 0, 1, false, { "@Mixin(UpdatedTarget.class)" })
  diagnostics.refresh(bufnr, { in_flight_policy = "latest", stale_result_policy = "drop" })
  helpers.assert_eq(#callbacks, 1)
  callbacks[1]({ result = { diagnostics = {} } }, nil)
  helpers.assert_eq(#callbacks, 2)
  callbacks[2]({
    result = {
      diagnostics = {
        {
          code = "LATEST",
          severity = "warning",
          message = "latest result",
          range = {
            start = { line = 0, character = 0 },
            ["end"] = { line = 0, character = 6 },
          },
        },
      },
    },
  }, nil)
  local published = vim.diagnostic.get(bufnr, { namespace = diagnostics.namespace })
  helpers.assert_eq(#published, 1)
  helpers.assert_eq(published[1].code, "LATEST")
  protocol_module.diagnostics = original_diagnostics
end)

with_named_buffer("/project/src/main/java/com/example/mixin/FailedDiagnosticsMixin.java", "java", {
  "@Mixin(SimpleTarget.class)",
}, function(bufnr)
  local original_get_clients = vim.lsp.get_clients
  local original_notify = vim.notify
  local request_count = 0
  local errors = {}
  local fake_client = {
    name = "jdtls",
    config = { root_dir = "/project" },
    request = function()
      request_count = request_count + 1
      return false, nil
    end,
  }
  vim.lsp.get_clients = function()
    return { fake_client }
  end
  vim.notify = function() end

  diagnostics.refresh(bufnr, {
    manual = true,
    callback = function(_, err)
      errors[#errors + 1] = err
    end,
  })
  helpers.assert_eq(request_count, 1)
  helpers.assert_eq(#errors, 1)

  diagnostics.refresh(bufnr, {
    manual = true,
    callback = function(_, err)
      errors[#errors + 1] = err
    end,
  })
  helpers.assert_eq(request_count, 2)
  helpers.assert_eq(#errors, 2)

  vim.lsp.get_clients = original_get_clients
  vim.notify = original_notify
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
    request = function(self, method, params, handler, request_bufnr)
      helpers.assert_eq(method, "workspace/executeCommand")
      commands_seen[params.command] = true
      if params.command == "mcdev.info" then
        handler(nil, {
          result = {
            lines = { "Extension loaded: true" },
            buildCommit = "test-commit",
            buildTime = "test-time",
            jarLocation = "file:///test.jar",
            registeredCommands = { "mcdev.info", "mcdev.completion" },
          },
        })
      elseif params.command == "mcdev.completion" then
        handler(nil, {
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
        handler(nil, { result = {} })
      end
      return true, 1
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

do
  local original_buf_request_all = vim.lsp.buf_request_all
  local original_mcdev_code_actions = code_action.code_actions
  vim.lsp.buf_request_all = function(_, method, _, callback)
    helpers.assert_eq(method, "textDocument/codeAction")
    callback({
      [1] = {
        result = {
          { title = "Standard fix", kind = "quickfix" },
        },
      },
    })
  end
  code_action.code_actions = function(_, _, _, callback)
    callback({
      { title = "mcdev fix", kind = "quickfix.mcdev" },
      { title = "Standard fix", kind = "quickfix" },
    }, nil)
  end
  local merged = nil
  lsp_adapter.code_actions(0, nil, {}, function(actions, err)
    helpers.assert_nil(err)
    merged = actions
  end)
  helpers.assert_not_nil(merged)
  helpers.assert_eq(#merged, 2)
  helpers.assert_eq(merged[1].title, "Standard fix")
  helpers.assert_eq(merged[2].title, "mcdev fix")
  vim.lsp.buf_request_all = original_buf_request_all
  code_action.code_actions = original_mcdev_code_actions
end

do
  local jar = vim.fn.tempname()
  vim.fn.writefile({}, jar)
  local original_jobstart = vim.fn.jobstart
  local original_jobstop = vim.fn.jobstop
  local original_chansend = vim.fn.chansend
  local original_timeout = stdio.timeout_ms
  local jobs = {}
  local next_job = 0

  vim.fn.jobstart = function(_, opts)
    next_job = next_job + 1
    jobs[next_job] = { opts = opts }
    return next_job
  end
  vim.fn.jobstop = function() end
  vim.fn.chansend = function(job, encoded)
    jobs[job].request = vim.json.decode(encoded)
    return #encoded
  end
  stdio.timeout_ms = 1
  stdio.stop("mcdev: test reset")

  local first_error = nil
  stdio.request({}, function(_, err)
    first_error = err
  end, { extension_jar = jar, java = "java" })
  vim.wait(100, function()
    return first_error ~= nil
  end, 10)
  helpers.assert_eq(first_error, "mcdev: stdio helper timed out")
  helpers.assert_eq(next_job, 1)

  local second_result = nil
  local second_error = nil
  stdio.request({}, function(result, err)
    second_result = result
    second_error = err
  end, { extension_jar = jar, java = "java" })
  helpers.assert_eq(next_job, 2)
  local request = jobs[2].request
  jobs[2].opts.on_stdout(2, {
    vim.json.encode({
      id = request.id,
      handled = true,
      response = { result = { items = {} } },
    }),
    "",
  })
  helpers.assert_not_nil(second_result)
  helpers.assert_nil(second_error)

  stdio.stop("mcdev: test cleanup")
  stdio.timeout_ms = original_timeout
  vim.fn.jobstart = original_jobstart
  vim.fn.jobstop = original_jobstop
  vim.fn.chansend = original_chansend
  vim.fn.delete(jar)
end

dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/protocol_positions.lua")
dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/diagnostic_positions.lua")
dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/jdtls_workspace.lua")
dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/blink_positions.lua")
dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/cmp_positions.lua")
dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/transport.lua")
print("mcdev-nvim adapter tests passed")
vim.cmd("qa!")
