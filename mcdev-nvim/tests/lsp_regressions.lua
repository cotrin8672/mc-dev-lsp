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
      commands[#commands + 1] = { client = self.id, command = command, bufnr = ctx.bufnr, params = ctx.params }
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
local function respond(request, result, err, context)
  local ctx = { client_id = request.client.id, bufnr = request.bufnr }
  if context then ctx = vim.tbl_extend("force", ctx, context) end
  request.callback(err, result, ctx)
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
    } } }, { lnum = 1, col = 0, end_col = 1,
    message = "unrelated", code = "UNRELATED" } })
  lsp.code_actions(0, explicit, {}, function() end)
  vim.diagnostic.reset(namespace, bufnr)
  for i, column in ipairs({ 10, 4, 3 }) do
    h.assert_eq(#pending[i].params.context.diagnostics, 1, "exclude unrelated diagnostics")
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

  local previous_selection = vim.o.selection
  local previous_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local previous_cursor = vim.api.nvim_win_get_cursor(0)
  local visual_line = "prefix left 😀"
  local first_col = #"prefix "
  local emoji_col = first_col + #"left "
  local emoji_end = vim.str_utfindex(visual_line, "utf-16", emoji_col + #"😀", false)
  local function leave_visual()
    if vim.api.nvim_get_mode().mode ~= "n" then
      vim.api.nvim_feedkeys(vim.api.nvim_replace_termcodes("<Esc>", true, false, true), "nx", false)
    end
  end
  local function assert_visual_range(mode, anchor, cursor, selection, expected)
    vim.o.selection = selection
    vim.api.nvim_win_set_cursor(0, anchor)
    vim.api.nvim_feedkeys(mode, "nx", false)
    h.assert_eq(vim.api.nvim_get_mode().mode, mode, "live visual mode")
    vim.api.nvim_win_set_cursor(0, cursor)
    vim.fn.maparg("<leader>ca", "v", false, true).callback()
    h.assert_eq(mcdev_range.start.line, expected.start.line)
    h.assert_eq(mcdev_range.start.character, expected.start.character)
    h.assert_eq(mcdev_range["end"].line, expected["end"].line)
    h.assert_eq(mcdev_range["end"].character, expected["end"].character)
    leave_visual()
  end
  local visual_ok, visual_err = pcall(function()
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { visual_line, "stale", "third" })
    vim.api.nvim_buf_set_mark(bufnr, "<", 2, 0, {})
    vim.api.nvim_buf_set_mark(bufnr, ">", 2, 2, {})
    assert_visual_range("v", { 1, first_col }, { 1, emoji_col }, "inclusive", {
      start = { line = 0, character = first_col },
      ["end"] = { line = 0, character = emoji_end },
    })
    assert_visual_range("v", { 1, emoji_col }, { 1, first_col }, "inclusive", {
      start = { line = 0, character = first_col },
      ["end"] = { line = 0, character = emoji_end },
    })
    assert_visual_range("v", { 1, first_col }, { 1, emoji_col }, "exclusive", {
      start = { line = 0, character = first_col },
      ["end"] = { line = 0, character = emoji_col },
    })
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, { "first", "second", "third" })
    assert_visual_range("V", { 1, 0 }, { 3, 0 }, "inclusive", {
      start = { line = 0, character = 0 },
      ["end"] = { line = 2, character = #"third" },
    })
  end)
  leave_visual()
  vim.o.selection = previous_selection
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, previous_lines)
  vim.api.nvim_win_set_cursor(0, previous_cursor)
  if not visual_ok then error(visual_err) end
  pending = {}
  lsp.code_actions(bufnr, nil, {}, function() end)
  h.assert_eq(pending[2].params.range.start.character, 4, "omitted range uses cursor")
end)

check("code action diagnostics follow range boundaries", function()
  clients = { client(1, "utf-8"), client(2, "utf-16"), client(3, "utf-32") }
  local namespace = vim.api.nvim_create_namespace("mcdev-lsp-range-boundaries")
  local function diagnostic(start_line, start_character, end_line, end_character, message)
    local start_text = vim.api.nvim_buf_get_lines(bufnr, start_line, start_line + 1, false)[1] or ""
    local end_text = vim.api.nvim_buf_get_lines(bufnr, end_line, end_line + 1, false)[1] or ""
    return {
      lnum = start_line,
      col = vim.str_byteindex(start_text, "utf-16", start_character, false),
      end_lnum = end_line,
      end_col = vim.str_byteindex(end_text, "utf-16", end_character, false),
      message = message,
    }
  end
  local function has_message(items, message)
    for _, item in ipairs(items) do
      if item.message == message then return true end
    end
    return false
  end
  local function request(range)
    pending = {}
    lsp.code_actions(bufnr, range, {}, function() end)
    h.assert_eq(#pending, 3, "one code action request per client")
  end
  local ok, err = pcall(function()
    vim.diagnostic.set(namespace, bufnr, {
      diagnostic(0, 3, 0, 4, "touches start"),
      diagnostic(0, 4, 0, 5, "inside"),
      diagnostic(0, 5, 0, 6, "touches end"),
      diagnostic(0, 4, 1, 0, "crosses line"),
    })
    request({ start = { line = 0, character = 4 }, ["end"] = { line = 0, character = 5 } })
    for _, request_item in ipairs(pending) do
      local items = request_item.params.context.diagnostics
      h.assert_eq(#items, 2, "exclude boundary-only diagnostics")
      h.assert_true(has_message(items, "inside"))
      h.assert_true(has_message(items, "crosses line"))
      h.assert_true(not has_message(items, "touches start"))
      h.assert_true(not has_message(items, "touches end"))
    end

    vim.diagnostic.set(namespace, bufnr, {
      diagnostic(0, 3, 0, 4, "point end"),
      diagnostic(0, 4, 0, 5, "point start"),
      diagnostic(0, 4, 0, 4, "point diagnostic"),
      diagnostic(0, 5, 0, 6, "point after"),
    })
    request({ start = { line = 0, character = 4 }, ["end"] = { line = 0, character = 4 } })
    for i, column in ipairs({ 10, 4, 3 }) do
      local items = pending[i].params.context.diagnostics
      h.assert_eq(#items, 2, "point uses half-open diagnostic ranges")
      h.assert_true(has_message(items, "point start"))
      h.assert_true(has_message(items, "point diagnostic"))
      h.assert_eq(pending[i].params.range.start.character, column, "point start encoding")
      h.assert_eq(pending[i].params.range["end"].character, column, "point end encoding")
    end
  end)
  vim.diagnostic.reset(namespace, bufnr)
  if not ok then error(err) end
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

check("code action command keeps request context", function()
  clients = { client(1, "utf-16") }
  actions.code_actions = function(_, _, _, cb) cb({}, nil) end
  local merged
  lsp.code_actions(bufnr, range(4), {}, function(items) merged = items end)
  local expected_params = vim.deepcopy(pending[1].params)
  local response_params = vim.deepcopy(expected_params)
  respond(pending[1], {
    { title = "Extract", kind = "refactor.extract", command = {
      command = "java.action.applyRefactoringCommand", arguments = { "extract" },
    } },
  }, nil, { params = response_params })
  response_params.range.start.character = 99
  pending[1].params.range.start.character = 99
  h.assert_eq(#merged, 1)

  actions.apply(merged[1], bufnr)
  local resolve = pending[2]
  h.assert_nil(resolve.params._mcdev_client_id, "resolve omits client routing metadata")
  h.assert_nil(resolve.params._mcdev_request_params, "resolve omits request snapshot metadata")
  h.assert_nil(resolve.params.params, "resolve does not add an outer context")
  vim.api.nvim_win_set_cursor(0, { 2, 0 })
  respond(resolve, { command = {
    command = "java.action.applyRefactoringCommand", arguments = { "extract" },
  } })
  h.assert_eq(#commands, 1)
  h.assert_true(vim.deep_equal(commands[1].params, expected_params),
    "exec_cmd receives the original code action params")
end)

check("JDT code action aliases", function()
  actions.code_actions = function(_, _, _, cb) cb({}, nil) end
  clients = { client(1, "utf-16") }
  clients[1].name = "jdtls"

  local function request(response)
    pending = {}
    local merged
    lsp.code_actions(bufnr, range(4), {}, function(items) merged = items end)
    h.assert_eq(#pending, 1)
    respond(pending[1], response)
    return merged
  end

  local command = {
    command = "java.action.overrideMethodsPrompt",
    arguments = { { textDocument = { uri = vim.uri_from_bufnr(bufnr) } } },
  }
  local merged = request({
    { title = "Generate Constructors", kind = "quickassist", data = { pid = "1" } },
    { title = "Generate Constructors", kind = "source.generate.constructors", data = { pid = "5" } },
    { title = "Generate toString()", kind = "quickassist", data = { pid = "2" } },
    { title = "Generate toString()", kind = "source.generate.toString", data = { pid = "6" } },
    { title = "Override/Implement Methods...", kind = "quickassist", command = command },
    { title = "Override/Implement Methods...", kind = "source.overrideMethods", command = command },
    { title = "Sort Members for 'DrillBlockMixin.java'", kind = "quickassist", data = { pid = "3" } },
    { title = "Sort Members for 'DrillBlockMixin.java'", kind = "source.sortMembers", data = { pid = "7" } },
  })
  h.assert_eq(#merged, 4, "known JDT aliases collapse without resolving")
  h.assert_eq(merged[1].data.pid, "1", "first alias remains lazy")
  h.assert_nil(merged[1].edit, "aliases are not resolved to render the picker")

  merged = request({
    { title = "Generate Constructors", kind = "quickassist",
      edit = { changes = { first = "constructor" } } },
    { title = "Generate Constructors", kind = "source.generate.constructors",
      edit = { changes = { second = "constructor" } } },
    { title = "Override/Implement Methods...", kind = "quickassist",
      command = { command = "java.action.overrideMethodsPrompt", arguments = { 1 } } },
    { title = "Override/Implement Methods...", kind = "source.overrideMethods",
      command = { command = "java.action.overrideMethodsPrompt", arguments = { 2 } } },
  })
  h.assert_eq(#merged, 4, "different edits and arguments remain distinct")

  merged = request({
    { title = "Command one", kind = "quickassist",
      command = "java.action.test", arguments = { 1 } },
    { title = "Command one alias", kind = "source.test",
      command = "java.action.test", arguments = { 1 } },
    { title = "Command two", kind = "quickassist",
      command = "java.action.test", arguments = { 2 } },
    { title = "Disabled command", kind = "quickassist",
      command = "java.action.test", arguments = { 2 }, disabled = { reason = "disabled" } },
    { title = "Disabled command alias", kind = "source.test",
      command = "java.action.test", arguments = { 2 }, disabled = { reason = "disabled" } },
  })
  h.assert_eq(#merged, 3, "top-level command arguments and disabled state affect identity")
  h.assert_eq(merged[1].arguments[1], 1, "equal top-level command arguments collapse")
  h.assert_eq(merged[2].arguments[1], 2, "different top-level command arguments remain distinct")
  h.assert_true(merged[3].disabled ~= nil, "disabled command remains distinct")

  clients = { client(1, "utf-16"), client(2, "utf-16") }
  clients[1].name, clients[2].name = "jdtls", "jdtls"
  pending = {}
  merged = nil
  lsp.code_actions(bufnr, range(4), {}, function(items) merged = items end)
  respond(pending[1], {
    { title = "Generate Constructors", kind = "quickassist", data = { pid = "1" } },
  })
  respond(pending[2], {
    { title = "Generate Constructors", kind = "source.generate.constructors", data = { pid = "5" } },
  })
  h.assert_eq(#merged, 2, "aliases from different clients remain routable")
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
