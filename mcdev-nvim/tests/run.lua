local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")

local mcdev = require("mcdev")
local completion = require("mcdev.completion")
local protocol = require("mcdev.protocol")
local config = require("mcdev.config")
local blink = require("mcdev.blink")
local cmp = require("mcdev.cmp")

mcdev.setup({
  jdtls = {
    extension_jar = "build/libs/io.github.mcdev.jdtls.jar",
  },
  mappings = {
    preferred_at_target = "descriptor",
    mixin_class_insert = "import",
    inject_method_descriptor = "auto",
  },
})

helpers.assert_eq(mcdev.extension_jar(), "build/libs/io.github.mcdev.jdtls.jar")

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
helpers.assert_eq(payload.options.preferredAtTarget, "descriptor")
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

local blink_adapter = blink.new()
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
local cmp_source = cmp.new()
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

print("mcdev-nvim adapter tests passed")
vim.cmd("qa!")
