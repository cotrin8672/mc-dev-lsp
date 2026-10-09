return function(h, bufnr)
  local helpers = h.helpers
  local lsp = require("mcdev.lsp")
  local code_action = require("mcdev.code_action")
  local client = vim.lsp.get_clients({ bufnr = bufnr, name = "jdtls" })[1]
  helpers.assert_not_nil(client, "JDT code action alias check requires an attached jdtls client")

  local original_name = vim.api.nvim_buf_get_name(bufnr)
  helpers.assert_true(original_name ~= "", "JDT code action alias check requires a named fixture buffer")
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
    "class DrillBlockMixinActionAlias {",
    "    private int value;",
    "    private int sum(int left, int right) {",
    "        return left + right;",
    "    }",
    "}",
  })

  -- Keep the fixture's long source and diagnostics untouched. Replacing it
  -- with this short in-memory source can leave JDT diagnostics with ranges
  -- from the old document, which makes a diagnostics-bearing codeAction
  -- request fail inside JDT before the alias merge is exercised.
  local action_bufnr = vim.api.nvim_create_buf(false, true)
  local action_name = vim.fn.fnamemodify(original_name, ":h") .. "/DrillBlockMixinActionAlias.java"
  vim.api.nvim_buf_set_name(action_bufnr, action_name)
  vim.bo[action_bufnr].filetype = "java"
  vim.api.nvim_buf_set_lines(action_bufnr, 0, -1, false, source)
  vim.lsp.buf_attach_client(action_bufnr, client.id)
  local attached = vim.wait(15000, function()
    return #vim.lsp.get_clients({ bufnr = action_bufnr, name = "jdtls" }) > 0
  end, 100)
  helpers.assert_true(attached, "fresh JDT code action buffer did not attach to jdtls")
  local action_client = vim.lsp.get_clients({ bufnr = action_bufnr, name = "jdtls" })[1]

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
    helpers.assert_true(started, "raw JDT code action request did not start")
    local settled = vim.wait(15000, function() return done end, 100)
    if not settled and request_id then
      action_client:cancel_request(request_id)
    end
    helpers.assert_true(settled, "raw JDT code action request timed out")
    helpers.assert_nil(request_error, "raw JDT code action request failed")
    return result and result ~= vim.NIL and result or {}
  end

  local raw_actions = request_raw()
  local source_aliases = {
    ["source.generate.constructors"] = true,
    ["source.generate.toString"] = true,
    ["source.overrideMethods"] = true,
    ["source.sortMembers"] = true,
  }
  local raw_alias_titles = {}
  local by_title = {}
  for _, action in ipairs(raw_actions) do
    local title = action.title
    local kind = action.kind
    if title and kind then
      local kinds = by_title[title] or {}
      kinds[kind] = true
      by_title[title] = kinds
    end
  end
  for title, kinds in pairs(by_title) do
    if kinds.quickassist then
      for source_kind in pairs(source_aliases) do
        if kinds[source_kind] then
          raw_alias_titles[#raw_alias_titles + 1] = title
          break
        end
      end
    end
  end
  table.sort(raw_alias_titles)
  helpers.assert_true(
    #raw_alias_titles > 0,
    "real JDT code actions did not expose a known quickassist/source alias pair"
  )
  h.log_step("JDT action aliases raw=" .. tostring(#raw_actions)
    .. " titles=" .. table.concat(raw_alias_titles, " | "))

  local merged, merged_error
  lsp.code_actions(action_bufnr, range, {}, function(actions, err)
    merged, merged_error = actions, err
  end)
  local settled = vim.wait(15000, function() return merged ~= nil end, 100)
  helpers.assert_true(settled, "mcdev merged code action request timed out")
  helpers.assert_nil(merged_error, "mcdev merged code action request failed")
  for _, title in ipairs(raw_alias_titles) do
    local count = 0
    for _, action in ipairs(merged) do
      if action.title == title then count = count + 1 end
    end
    helpers.assert_eq(count, 1, "merged JDT alias should appear once: " .. title)
  end

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
  helpers.assert_not_nil(extract_action, "real JDT did not offer Extract to local variable")
  helpers.assert_true(
    type(extract_action.command) == "table"
      and extract_action.command.command == "java.action.applyRefactoringCommand",
    "Extract to local variable should use JDT's client-side refactoring command"
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
  helpers.assert_true(extracted, "JDT Extract to local variable was not applied")
  local extracted_source = table.concat(vim.api.nvim_buf_get_lines(action_bufnr, 0, -1, false), "\n")
  helpers.assert_nil(
    extracted_source:find("return left + right;", 1, true),
    "Extract to local variable should replace the selected expression"
  )

  vim.api.nvim_set_current_buf(bufnr)
  vim.api.nvim_buf_delete(action_bufnr, { force = true })
end
