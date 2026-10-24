return function(h, bufnr)
  local helpers = h.helpers
  local lsp = require("mcdev.lsp")
  local code_action = require("mcdev.code_action")
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "ktlsp" })[1]
  helpers.assert_not_nil(client, "host code action alias check requires an attached ktlsp client")

  local original_name = vim.api.nvim_buf_get_name(bufnr)
  helpers.assert_true(original_name ~= "", "host code action alias check requires a named fixture buffer")
  local original_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
  local package_line
  for _, line in ipairs(original_lines) do
    if line:match("^%s*package%s+[%w_.]+%s*;%s*$") then
      package_line = line
      break
    end
  end
  local source = {}
  if package_line then
    source[#source + 1] = package_line
    source[#source + 1] = ""
  end
  vim.list_extend(source, {
    "import java.util.Set;",
    "import java.util.List;",
    "",
    "class DrillBlockMixinActionAlias {",
    "    private int value;",
    "    private int sum(int left, int right) {",
    "        return left + right;",
    "    }",
    "}",
  })

  -- Keep the fixture's long source and diagnostics untouched. Replacing it
  -- with this short in-memory source can leave host diagnostics with ranges
  -- from the old document, which makes a diagnostics-bearing codeAction
  -- request fail inside host before the alias merge is exercised.
  local action_bufnr = vim.api.nvim_create_buf(false, true)
  local action_name = vim.fn.fnamemodify(original_name, ":h") .. "/DrillBlockMixinActionAlias.java"
  vim.api.nvim_buf_set_name(action_bufnr, action_name)
  vim.bo[action_bufnr].filetype = "java"
  vim.api.nvim_buf_set_lines(action_bufnr, 0, -1, false, source)
  vim.lsp.buf_attach_client(action_bufnr, client.id)
  local attached = vim.wait(15000, function()
    return #vim.lsp.get_clients({ bufnr = action_bufnr, name = "ktlsp" }) > 0
  end, 100)
  helpers.assert_true(attached, "fresh host code action buffer did not attach to ktlsp")
  local action_client = vim.lsp.get_clients({ bufnr = action_bufnr, name = "ktlsp" })[1]

  local range = {
    start = { line = 0, character = 0 },
    ["end"] = { line = #source - 1, character = #source[#source] },
  }
  local params = {
    textDocument = vim.lsp.util.make_text_document_params(action_bufnr),
    range = range,
    context = { diagnostics = {} },
  }

  local function request_raw()
    local done, result, request_error = false, nil, nil
    local started, request_id = action_client:request("textDocument/codeAction", params, function(err, response)
      request_error, result, done = err, response, true
    end, action_bufnr)
    helpers.assert_true(started, "raw host code action request did not start")
    local settled = vim.wait(15000, function() return done end, 100)
    if not settled and request_id then
      action_client:cancel_request(request_id)
    end
    helpers.assert_true(settled, "raw host code action request timed out")
    helpers.assert_nil(request_error, "raw host code action request failed")
    return result and result ~= vim.NIL and result or {}
  end

  local raw_actions = request_raw()
  -- Native ktlsp returns materialized WorkspaceEdits, without JDT's lazy aliases.
  -- The four original alias families remain exercised verbatim by unit tests.
  local native_action = vim.tbl_filter(function(action)
    return action.kind == "source.organizeImports" and type(action.edit) == "table"
  end, raw_actions)[1]
  helpers.assert_not_nil(native_action, "real host must expose its existing organize-imports WorkspaceEdit")
  helpers.assert_true(client.server_capabilities.codeActionProvider.resolveProvider == false,
    "native action fixture must use the advertised materialized-action surface")
  h.log_step("native materialized action raw=" .. tostring(#raw_actions) .. " title=" .. native_action.title)

  local merged, merged_error
  lsp.code_actions(action_bufnr, range, {}, function(actions, err)
    merged, merged_error = actions, err
  end)
  local settled = vim.wait(15000, function() return merged ~= nil end, 100)
  helpers.assert_true(settled, "mcdev merged code action request timed out")
  helpers.assert_nil(merged_error, "mcdev merged code action request failed")
  local matching_effects = vim.tbl_filter(function(action)
    return action.title == native_action.title and vim.deep_equal(action.edit, native_action.edit)
  end, merged)
  helpers.assert_eq(#matching_effects, 1, "native materialized effect should appear once in the merged picker")
  helpers.assert_eq(matching_effects[1]._mcdev_client_id, action_client.id,
    "materialized native action must stay routed to its originating client")

  local expression_line, expression_start
  for index, line in ipairs(source) do
    local start = line:find("left + right", 1, true)
    if start then
      expression_line = index - 1
      expression_start = start - 1
      break
    end
  end
  helpers.assert_not_nil(expression_line, "extract fixture expression not found")
  local extract_range = {
    start = { line = expression_line, character = expression_start },
    ["end"] = { line = expression_line, character = expression_start + #"left + right" },
  }
  local extract_actions, extract_error
  lsp.code_actions(action_bufnr, extract_range, {}, function(actions, err)
    extract_actions, extract_error = actions, err
  end)
  helpers.assert_true(
    vim.wait(15000, function() return extract_actions ~= nil end, 100),
    "mcdev extract code action request timed out"
  )
  helpers.assert_nil(extract_error, "mcdev extract code action request failed")
  local extract_action = vim.tbl_filter(function(action)
    local title = (action.title or ""):lower()
    return title:find("extract", 1, true) ~= nil and title:find("local variable", 1, true) ~= nil
  end, extract_actions)[1]
  helpers.assert_not_nil(extract_action, "real host did not offer Extract to local variable")
  helpers.assert_true(
    type(extract_action.edit) == "table",
    "Extract to local variable must provide a native WorkspaceEdit; the real host feature remains required"
  )

  vim.api.nvim_set_current_buf(action_bufnr)
  vim.api.nvim_win_set_cursor(0, { 1, 0 })
  extract_range.start.character = 0
  extract_range["end"].character = 0
  code_action.apply(extract_action, action_bufnr)

  local extracted = vim.wait(15000, function()
    local lines = vim.api.nvim_buf_get_lines(action_bufnr, 0, -1, false)
    local has_assignment, has_return = false, false
    for _, line in ipairs(lines) do
      has_assignment = has_assignment or line:find("=%s*left%s+%+%s+right%s*;", 1) ~= nil
      has_return = has_return or line:match("^%s*return%s+[%a_][%w_]*%s*;%s*$") ~= nil
    end
    return has_assignment and has_return
  end, 100)
  helpers.assert_true(extracted, "host Extract to local variable was not applied")
  local extracted_source = table.concat(vim.api.nvim_buf_get_lines(action_bufnr, 0, -1, false), "\n")
  helpers.assert_nil(
    extracted_source:find("return left + right;", 1, true),
    "Extract to local variable should replace the selected expression"
  )

  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_buf_delete(action_bufnr, { force = true })
end
