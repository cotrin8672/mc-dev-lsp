local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")
local root = vim.fs.normalize(vim.fn.tempname())
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, root .. "/src/RealClient.java")
vim.bo[bufnr].filetype = "java"
vim.bo[bufnr].fileformat = "dos"
vim.bo[bufnr].endofline = true
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class 猫 {", "}" })

local events, serial, closed = {}, 0, false
local client_id = vim.lsp.start({
  name = "ktlsp",
  root_dir = root,
  init_options = { analysisProviders = { ["mcdev.mixin"] = {
    command = { "companion" }, enabled = true, executeCommands = { protocol.commands.info },
  } } },
  cmd = function(dispatchers)
    return {
      request = function(method, params, callback, notify_reply)
        serial = serial + 1
        local request_id = serial
        events[#events + 1] = { method = method, params = vim.deepcopy(params) }
        local result
        if method == "initialize" then
          result = { capabilities = { textDocumentSync = { openClose = true, change = 1 },
            executeCommandProvider = { commands = { "ktlsp.providerCommand" } } } }
        elseif method == "workspace/executeCommand" then
          result = { result = { OpaqueResult = "猫" } }
        else
          result = vim.NIL
        end
        vim.schedule(function()
          callback(nil, result)
          if notify_reply then notify_reply(request_id) end
        end)
        return true, request_id
      end,
      notify = function(method, params)
        events[#events + 1] = { method = method, params = vim.deepcopy(params) }
        return true
      end,
      is_closing = function() return closed end,
      terminate = function()
        if not closed then
          closed = true
          vim.schedule(function() dispatchers.on_exit(0, 0) end)
        end
      end,
    }
  end,
}, { bufnr = bufnr, reuse_client = function() return false end })
h.assert_not_nil(client_id)
local client, resource_bufnr
local ok, failure = xpcall(function()
  h.assert_true(vim.wait(3000, function()
    client = vim.lsp.get_client_by_id(client_id)
    return client and client.initialized and client.attached_buffers[bufnr]
  end, 10), "real Neovim LSP client did not initialize")

  local function latest(method)
    for index = #events, 1, -1 do
      if events[index].method == method then return events[index], index end
    end
  end
  local opened = assert(latest("textDocument/didOpen")).params.textDocument
  local initial = protocol.context(bufnr, { 1, 1 })
  h.assert_eq(initial.bufferText, opened.text)
  h.assert_eq(initial.documentVersion, opened.version)
  h.assert_eq(opened.text, "class 猫 {\r\n}\r\n")

  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "class 猫 {", "  // 🙂", "}" })
  local request = { context = protocol.context(bufnr, { 1, 1 }), OpaqueField = "KeepCase" }
  local result, request_error
  protocol.request(protocol.commands.info, request, function(value, err) result, request_error = value, err end, bufnr)
  h.assert_true(vim.wait(3000, function() return result ~= nil or request_error ~= nil end, 10))
  h.assert_nil(request_error)
  h.assert_eq(result.result.OpaqueResult, "猫")
  local changed, changed_index = latest("textDocument/didChange")
  local executed, executed_index = latest("workspace/executeCommand")
  h.assert_not_nil(changed, "request must flush pending didChange")
  h.assert_not_nil(executed)
  h.assert_true(changed_index < executed_index, "didChange must reach host before the command")
  local outer = executed.params.arguments[1]
  h.assert_eq(outer.context.bufferText, changed.params.contentChanges[1].text)
  h.assert_eq(outer.context.documentVersion, changed.params.textDocument.version)
  h.assert_eq(outer.context.documentUri, changed.params.textDocument.uri)
  h.assert_eq(outer.context.bufferText, "class 猫 {\r\n  // 🙂\r\n}\r\n")
  h.assert_eq(outer.arguments[1].OpaqueField, "KeepCase")

  resource_bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(resource_bufnr, root .. "/src/mod.accesswidener")
  vim.bo[resource_bufnr].filetype = "accesswidener"
  vim.bo[resource_bufnr].fileformat = "unix"
  vim.bo[resource_bufnr].endofline = true
  vim.api.nvim_buf_set_lines(resource_bufnr, 0, -1, false, {
    "accessWidener v2 named", "accessible class game/Target",
  })
  h.assert_true(not client.attached_buffers[resource_bufnr])
  local resource_context = protocol.context(resource_bufnr, { 1, 1 })
  local resource_result, resource_error
  protocol.request(protocol.commands.info, { context = resource_context }, function(value, err)
    resource_result, resource_error = value, err
  end, resource_bufnr)
  h.assert_true(vim.wait(3000, function() return resource_result ~= nil or resource_error ~= nil end, 10))
  h.assert_nil(resource_error)
  h.assert_true(client.attached_buffers[resource_bufnr], "provider must own an unopened resource buffer")
  local resource_opened, opened_index = latest("textDocument/didOpen")
  local resource_command, resource_index = latest("workspace/executeCommand")
  h.assert_true(opened_index < resource_index, "resource didOpen must precede its command")
  local resource_outer = resource_command.params.arguments[1]
  h.assert_eq(resource_outer.context.documentUri, resource_opened.params.textDocument.uri)
  h.assert_eq(resource_outer.context.bufferText, resource_opened.params.textDocument.text)
  h.assert_eq(resource_outer.context.documentVersion, resource_opened.params.textDocument.version)
  h.assert_eq(resource_outer.context.bufferText, "accessWidener v2 named\naccessible class game/Target\n")
end, debug.traceback)
client = client or vim.lsp.get_client_by_id(client_id)
if client then client:stop(true) end
vim.wait(1000, function() return closed end, 10)
if resource_bufnr then vim.api.nvim_buf_delete(resource_bufnr, { force = true }) end
vim.api.nvim_buf_delete(bufnr, { force = true })
if not ok then error(failure) end
print("mcdev-nvim real Neovim LSP text and version routing tests passed")
