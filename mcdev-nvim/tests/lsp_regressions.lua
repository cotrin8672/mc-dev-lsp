local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local lsp = require("mcdev.lsp")
local navigation = require("mcdev.navigation")
local hover = require("mcdev.hover")
local actions = require("mcdev.code_action")
local config = require("mcdev.config")
local attach = require("mcdev.attach")

local original_buf = vim.api.nvim_get_current_buf()
local original_cursor = vim.api.nvim_win_get_cursor(0)
local original_get_clients = vim.lsp.get_clients
local original_get_client = vim.lsp.get_client_by_id
local original_actions = actions.code_actions
local original_notify = vim.notify
local original_options = vim.deepcopy(config.options)
local original_select = vim.ui.select
local bufnr = vim.api.nvim_create_buf(false, true)
vim.api.nvim_buf_set_name(bufnr, vim.fn.tempname() .. ".java")
vim.api.nvim_set_current_buf(bufnr)
local line = "日😀本target"
vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { line, line })
vim.api.nvim_win_set_cursor(0, { 1, #"日😀本" })

local clients, pending, commands, notices = {}, {}, {}, {}
local function client(id, encoding)
  return {
    id = id, name = "test" .. id, offset_encoding = encoding,
    supports_method = function() return true end,
    request = function(self, method, params, callback, buffer)
      pending[#pending + 1] = { client = self, method = method, params = params, callback = callback, bufnr = buffer }
      return true, #pending
    end,
    exec_cmd = function(self, command, ctx)
      commands[#commands + 1] = { client = self.id, command = command, bufnr = ctx.bufnr }
    end,
  }
end
vim.lsp.get_clients = function(filter)
  return vim.tbl_filter(function(c)
    return (not filter or not filter.name or c.name == filter.name)
      and (not filter or not filter.method or c:supports_method(filter.method))
  end, clients)
end
vim.lsp.get_client_by_id = function(id)
  for _, c in ipairs(clients) do if c.id == id then return c end end
end
vim.notify = function(message) notices[#notices + 1] = message end
local function respond(request, result, err)
  request.callback(err, result, { client_id = request.client.id, bufnr = request.bufnr })
end
local function range(column)
  return { start = { line = 0, character = column }, ["end"] = { line = 0, character = column + 1 } }
end
local failures = {}
local function check(name, fn)
  pending, commands, notices = {}, {}, {}
  local ok, err = xpcall(fn, debug.traceback)
  if not ok then failures[#failures + 1] = name .. ": " .. err end
end

check("per-client positions", function()
  clients = { client(1, "utf-8"), client(2, "utf-16"), client(3, "utf-32") }
  for _, method in ipairs({ "definition", "references", "hover" }) do
    pending = {}
    lsp[method](0, { 1, #"日😀本" }, function() end)
    for i, column in ipairs({ 10, 4, 3 }) do
      h.assert_eq(pending[i].params.position.character, column, method .. " encoding")
      h.assert_eq(pending[i].params.position.line, 0)
    end
    if method == "references" then h.assert_true(pending[1].params.context.includeDeclaration) end
  end
end)

check("single completion across LSP responses", function()
  for _, method in ipairs({ "definition", "references", "hover" }) do
    local provider = method == "hover" and hover or navigation
    local original = provider[method]
    local ok, err = pcall(function()
      for _, responses in ipairs({ { "hit", "empty" }, { "empty", "hit" }, { "hit", "hit" },
        { "error", "hit" }, { "empty", "error" }, { "null", "empty" } }) do
        clients = { client(1, "utf-16"), client(2, "utf-8") }
        pending = {}
        local callbacks, fallbacks = 0, 0
        provider[method] = function(_, _, cb)
          fallbacks = fallbacks + 1
          cb({}, nil)
        end
        lsp[method](bufnr, { 1, 0 }, function() callbacks = callbacks + 1 end)
        local requests = vim.list_slice(pending)
        for i, response in ipairs(responses) do
          local result = response == "hit" and (method == "hover" and { contents = { "hover" } }
            or { { uri = vim.uri_from_bufnr(bufnr), range = range(4) } })
            or (response == "null" and vim.NIL or nil)
          respond(requests[i], result, response == "error" and { message = "failed" } or nil)
          if i == 1 and response ~= "hit" then h.assert_eq(fallbacks, 0, "wait for other client") end
          if i == 1 and response == "hit" then h.assert_eq(callbacks, 1, "do not wait for slower clients after success") end
        end
        h.assert_eq(callbacks, 1, method .. " completes once")
        h.assert_eq(fallbacks, vim.tbl_contains(responses, "hit") and 0 or 1)
      end
      clients, pending = {}, {}
      local called = 0
      provider[method] = function() called = called + 1 end
      lsp[method](bufnr, nil, function() end)
      h.assert_eq(called, 1, "no clients must fall back")
      clients = { client(1, "utf-16") }
      clients[1].request = function() return false end
      lsp[method](bufnr, nil, function() end)
      h.assert_eq(called, 2, "failed request dispatch must fall back")
    end)
    provider[method] = original
    if not ok then error(err) end
  end
end)

check("navigation uses result encoding", function()
  config.options.navigation = { enable = true }
  config.options.code_action = { enable = true }
  config.options.standard_lsp.prefer = true
  attach.setup(bufnr)
  for _, encoding in ipairs({ "utf-8", "utf-16", "utf-32" }) do
    clients = { client(1, encoding) }
    for _, key in ipairs({ "gd", "gr" }) do
      for _, count in ipairs({ 1, 2 }) do
        pending = {}
        vim.api.nvim_win_set_cursor(0, { 1, 0 })
        local mapping = vim.fn.maparg(key, "n", false, true)
        mapping.callback()
        local column = encoding == "utf-8" and 10 or (encoding == "utf-16" and 4 or 3)
        local locations = { { uri = vim.uri_from_bufnr(bufnr), range = range(column) } }
        if count == 2 then locations[2] = vim.deepcopy(locations[1]) end
        vim.ui.select = function(items, _, cb) cb(items[#items]) end
        respond(pending[1], locations)
        h.assert_eq(vim.api.nvim_win_get_cursor(0)[2], 10, key .. " target column")
      end
    end
    pending = {}
    vim.fn.maparg("gd", "n", false, true).callback()
    local column = encoding == "utf-8" and 10 or (encoding == "utf-16" and 4 or 3)
    respond(pending[1], { { targetUri = vim.uri_from_bufnr(bufnr), targetRange = range(0),
      targetSelectionRange = range(column) } })
    h.assert_eq(vim.api.nvim_win_get_cursor(0)[2], 10, "LocationLink selection")
  end
  local original = navigation.definition
  config.options.standard_lsp.prefer = false
  navigation.definition = function(_, _, cb)
    cb({ { uri = vim.uri_from_bufnr(bufnr), range = range(4) } }, nil)
  end
  local ok, err = pcall(function()
    vim.fn.maparg("gd", "n", false, true).callback()
    h.assert_eq(vim.api.nvim_win_get_cursor(0)[2], 10, "mcdev targets are UTF-16")
  end)
  navigation.definition = original
  if not ok then error(err) end
end)

check("code action request ranges", function()
  clients = { client(1, "utf-8"), client(2, "utf-16"), client(3, "utf-32") }
  local mcdev_range
  actions.code_actions = function(_, r, _, cb) mcdev_range = r; cb({}, nil) end
  local explicit = range(4)
  local namespace = vim.api.nvim_create_namespace("mcdev-lsp-regressions")
  vim.diagnostic.set(namespace, bufnr, { { lnum = 0, col = 10, end_col = 11,
    message = "diagnostic", code = "TEST", user_data = { lsp = {
      message = "diagnostic", code = "TEST", data = { token = 1 },
    } } } })
  lsp.code_actions(0, explicit, {}, function() end)
  vim.diagnostic.reset(namespace, bufnr)
  for i, column in ipairs({ 10, 4, 3 }) do
    h.assert_eq(pending[i].params.range.start.character, column)
    h.assert_eq(pending[i].params.range["end"].character, column + 1)
    local diagnostic = pending[i].params.context.diagnostics[1]
    h.assert_eq(diagnostic.range.start.character, column)
    h.assert_eq(diagnostic.range["end"].character, column + 1)
    h.assert_eq(diagnostic.data.token, 1)
    h.assert_nil(diagnostic.lnum, "LSP diagnostics are not vim.Diagnostic objects")
  end
  h.assert_eq(explicit.start.character, 4, "caller range remains UTF-16")
  config.options.code_action = { enable = true }
  config.options.standard_lsp.prefer = false
  attach.setup(bufnr)
  vim.api.nvim_win_set_cursor(0, { 1, 10 })
  vim.fn.maparg("<leader>ca", "n", false, true).callback()
  h.assert_eq(mcdev_range.start.character, 4)
  h.assert_eq(mcdev_range["end"].character, 4)
  for _, end_byte in ipairs({ 3, 2147483647 }) do
    vim.api.nvim_buf_set_mark(bufnr, "<", 1, 0, {})
    vim.api.nvim_buf_set_mark(bufnr, ">", 2, end_byte, {})
    vim.fn.maparg("<leader>ca", "v", false, true).callback()
    h.assert_eq(mcdev_range.start.character, 0)
    h.assert_eq(mcdev_range["end"].line, 1)
    h.assert_eq(mcdev_range["end"].character, end_byte == 3 and 3 or 10, "whole emoji or line end")
  end
  pending = {}
  lsp.code_actions(bufnr, nil, {}, function() end)
  h.assert_eq(pending[2].params.range.start.character, 4, "omitted range uses cursor")
end)

check("plain commands, edits and resolve failures", function()
  clients = { client(1, "utf-8") }
  local plain = { title = "Run", command = "test.run", arguments = { 7 }, _mcdev_client_id = 1 }
  actions.apply(plain, 0)
  h.assert_eq(#pending, 0, "plain Commands must not be resolved")
  h.assert_eq(commands[1].client, 1)
  h.assert_eq(commands[1].command.arguments[1], 7)
  local edit = { changes = { [vim.uri_from_bufnr(bufnr)] = { { range = range(10), newText = "T" } } } }
  clients[1].supports_method = function() return false end
  actions.apply({ title = "Edit", edit = edit, _mcdev_client_id = 1 }, bufnr)
  h.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], "日😀本Target")
  h.assert_eq(#pending, 0, "unsupported resolve applies existing edit")
  clients[1].supports_method = function() return true end
  actions.apply({ title = "Run", command = { command = "test.run" }, _mcdev_client_id = 1 }, bufnr)
  respond(pending[1], nil, { message = "resolve failed" })
  h.assert_eq(#commands, 2, "preserve executable action when resolve fails")
  actions.apply({ title = "Disabled", command = "test.run", disabled = { reason = "disabled" }, _mcdev_client_id = 1 }, bufnr)
  h.assert_eq(#commands, 2)
  clients = {}
  actions.apply(plain, bufnr)
  h.assert_eq(#commands, 2, "never reroute actions from a disconnected client")
  clients = { client(2, "utf-8") }
  clients[1].name = "jdtls"
  edit.changes[vim.uri_from_bufnr(bufnr)][1] = { range = range(4), newText = "t" }
  actions.apply({ title = "mcdev", edit = edit }, bufnr)
  h.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], line, "mcdev edits always use UTF-16")
end)

check("code action identity and resolution", function()
  clients = { client(1, "utf-8"), client(2, "utf-16") }
  actions.code_actions = function(_, _, _, cb) cb({}, nil) end
  local merged
  lsp.code_actions(bufnr, range(4), {}, function(items) merged = items end)
  respond(pending[1], { { title = "Fix", kind = "quickfix", data = { id = 1 } },
    { title = "Fix", kind = "quickfix", data = { id = 2 } } })
  respond(pending[2], { { title = "Fix", kind = "quickfix", data = { id = 1 } } })
  h.assert_eq(#merged, 3, "same-title actions keep data and origin")
  local selected = merged[1]
  local selected_data = vim.deepcopy(selected.data)
  actions.apply(selected, bufnr)
  h.assert_eq(#pending, 3, "selected action is resolved")
  local resolve = pending[3]
  h.assert_eq(resolve.method, "codeAction/resolve")
  h.assert_true(vim.deep_equal(resolve.params.data, selected_data))
  h.assert_nil(resolve.params._mcdev_client_id, "private routing data is not sent")
  local encoding = resolve.client.offset_encoding
  local column = encoding == "utf-8" and 10 or 4
  respond(resolve, { title = "Fix", edit = { changes = { [vim.uri_from_bufnr(bufnr)] = {
    { range = range(column), newText = "T" },
  } } }, command = { title = "after", command = "test.after", arguments = { 42 } } })
  h.assert_eq(vim.api.nvim_buf_get_lines(bufnr, 0, 1, false)[1], "日😀本Target")
  h.assert_eq(#commands, 1)
  h.assert_eq(commands[1].client, resolve.client.id, "execute on the originating client")
  h.assert_eq(commands[1].bufnr, bufnr)
  h.assert_eq(commands[1].command.arguments[1], 42)
  actions.apply(merged[2], bufnr)
  respond(pending[4], nil, { message = "resolve failed" })
  h.assert_eq(#commands, 1, "failed resolve does not execute")
  h.assert_true(#notices > 0, "failed resolve is visible")
end)

check("mcdev actions without standard clients", function()
  clients = {}
  actions.code_actions = function(_, _, _, cb) cb({ { title = "mcdev fix" } }, nil) end
  local result
  lsp.code_actions(bufnr, nil, {}, function(items) result = items end)
  h.assert_not_nil(result, "no standard clients must not stall merge")
  h.assert_eq(#result, 1)
end)

vim.lsp.get_clients = original_get_clients
vim.lsp.get_client_by_id = original_get_client
actions.code_actions = original_actions
vim.notify = original_notify
vim.ui.select = original_select
config.options = original_options
vim.api.nvim_set_current_buf(original_buf)
if original_cursor[1] > 0 then vim.api.nvim_win_set_cursor(0, original_cursor) end
vim.api.nvim_buf_delete(bufnr, { force = true })
if #failures > 0 then error(table.concat(failures, "\n")) end
print("mcdev-nvim LSP regression tests passed")
