return function(h)
  local completion = require("mcdev.completion")
  h.with_buffer(h.mixin_file, "java", nil, function(bufnr)
    local original = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local line_number, character
    local prefix = 'method = "renderCameraOverl'
    for index, line in ipairs(original) do
      local start = line:find(prefix, 1, true)
      if start then
        line_number, character = index, start - 1 + #prefix
        break
      end
    end
    h.helpers.assert_not_nil(line_number, "real GuiMixin method selector must exist")
    -- Keep the real Gradle project, dependencies and source. Only simulate an
    -- unfinished selector in the editor; never save the incomplete source.
    for _, phase in ipairs({ "startup", "warm" }) do
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, original)
      vim.api.nvim_buf_set_lines(bufnr, line_number - 1, line_number, false, {
        (original[line_number]:gsub("renderCameraOverlays", "renderCameraOverl", 1)),
      })
      local started = vim.uv.hrtime()
      local first_ms, final, final_ms
      local hover_done, hover_result, hover_err, hover_ms = phase ~= "startup"
      local hover_client, hover_request
      if phase == "startup" then
        hover_client = vim.lsp.get_clients({ bufnr = bufnr, name = "jdtls" })[1]
        h.helpers.assert_not_nil(hover_client, "real Java hover requires the attached JDT client")
        local hover_position
        for index, line in ipairs(original) do
          local start = line:find("@Mixin(Gui.class)", 1, true)
          if start then hover_position = { line = index - 1, character = start - 1 + #"@Mixin(G" } end
        end
        h.helpers.assert_not_nil(hover_position)
        local requested
        requested, hover_request = hover_client:request("textDocument/hover", {
          textDocument = { uri = vim.uri_from_bufnr(bufnr) }, position = hover_position,
        }, function(err, result)
          hover_done, hover_err, hover_result = true, err, result
          hover_ms = (vim.uv.hrtime() - started) / 1e6
        end, bufnr)
        h.helpers.assert_true(requested, "standard Java hover request must start")
      end
      completion.last_debug = nil
      local cancel = completion.complete(function(result)
        local elapsed = (vim.uv.hrtime() - started) / 1e6
        first_ms = first_ms or elapsed
        if not result.isProvisional then
          final, final_ms = result, elapsed
        end
      end, bufnr, { line_number, character }, { source = "real-sodium-" .. phase, stream = true })
      local settled = vim.wait(120000, function() return final ~= nil and hover_done end, 100)
      if not settled then
        cancel()
        if hover_request then hover_client:cancel_request(hover_request) end
      end
      h.log_step(string.format("Sodium %s first_ms=%.1f final_ms=%.1f", phase, first_ms or -1, final_ms or -1))
      if phase == "startup" then
        h.log_step(string.format("Sodium standard Java hover startup_ms=%.1f", hover_ms or -1))
        h.helpers.assert_nil(hover_err, "standard Java hover failed")
        h.helpers.assert_true(hover_result ~= nil and hover_result.contents ~= nil
          and vim.inspect(hover_result.contents):find("Gui", 1, true) ~= nil,
          "standard Java hover must resolve the actual Minecraft Gui class: " .. vim.inspect(hover_result))
      end
      h.log_step("Sodium " .. phase .. " transport_error=" .. vim.inspect(completion.last_project_transport_error)
        .. " debug=" .. vim.inspect(completion.last_debug))
      h.helpers.assert_true(settled, "real project completion did not settle within 120s")
      local debug = completion.last_debug
      h.helpers.assert_true(debug ~= nil and completion.last_project_transport_error == nil,
        "final response must come from the project-aware transport: "
          .. vim.inspect({ debug = completion.last_debug, error = completion.last_error, result = final }))
      -- usedJavaProject describes AST binding, not the origin of the member index.
      -- The guarded project transport may correctly use its full bytecode index
      -- when JDT AST parsing is temporarily unavailable.
      if debug.parseSource == "JDT_AST" then
        h.helpers.assert_true(debug.usedJavaProject == true and debug.usedCompilationUnit == true,
          "AST response must resolve the actual project compilation unit")
      else
        h.helpers.assert_eq(debug.parseSource, "HAND_WRITTEN_FALLBACK")
        h.helpers.assert_true(type(debug.fallbackReason) == "string" and debug.fallbackReason:find("%S") ~= nil,
          "project bytecode fallback must expose its cause")
      end
      local candidates = vim.tbl_filter(function(item)
        return item.insertText == "renderCameraOverlays"
      end, final.items or {})
      h.helpers.assert_eq(#candidates, 1, "final project-aware completion must resolve Gui.renderCameraOverlays: " .. vim.inspect(final))
      local item = candidates[1]
      h.helpers.assert_not_nil(item.textEdit, "real project completion must provide its edit")
      local edits = { item.textEdit }
      vim.list_extend(edits, item.additionalTextEdits or {})
      vim.lsp.util.apply_text_edits(edits, bufnr, "utf-16")
      h.helpers.assert_eq(table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
        table.concat(original, "\n"), "real project edit must exactly restore the original source")
    end
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, original)
    vim.bo[bufnr].modified = false
  end)
end
