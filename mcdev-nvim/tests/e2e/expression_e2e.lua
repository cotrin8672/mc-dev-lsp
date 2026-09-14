return function(h)
  local helpers = h.helpers
  local completion = require("mcdev.completion")
  local navigation = require("mcdev.navigation")
  h.with_buffer(h.mixin_file, "java", {
    "package com.example.mixin;",
    "import com.example.target.SimpleTarget;",
    "import com.llamalad7.mixinextras.expression.Expression;",
    "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
    "import org.spongepowered.asm.mixin.Mixin;",
    "import org.spongepowered.asm.mixin.injection.At;",
    "@Mixin(SimpleTarget.class)",
    "public abstract class MixinExtrasExample {",
    '    @Expression("this.")',
    '    @ModifyExpressionValue(method = "compute()I", at = @At("MIXINEXTRAS:EXPRESSION"))',
    "    private int modifyCounter(int original) { return original; }",
    "}",
  }, function(bufnr)
    local function position_at(marker)
      for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
        local start = line:find(marker, 1, true)
        if start then
          return { index, start - 1 + #marker }
        end
      end
      error("expression E2E marker missing: " .. marker)
    end

    local function at(marker)
      return h.build_context(bufnr, position_at(marker))
    end

    local function navigation_locations(marker)
      local locations, navigation_err
      navigation.definition(bufnr, position_at(marker), function(found, err)
        locations = found
        navigation_err = err
      end)
      helpers.assert_true(vim.wait(60000, function()
        return locations ~= nil or navigation_err ~= nil
      end, 50), "expression navigation adapter did not settle")
      helpers.assert_nil(navigation_err, "expression navigation adapter failed")
      return locations
    end

    local function show_location(location, message)
      local expected_line = location.range.start.line + 1
      vim.lsp.util.show_document(location, "utf-8", { focus = true })
      helpers.assert_true(vim.wait(10000, function()
        local current_buf = vim.api.nvim_get_current_buf()
        local cursor = vim.api.nvim_win_get_cursor(0)
        return vim.uri_from_bufnr(current_buf) == location.uri and cursor[1] == expected_line
          and cursor[2] == location.range.start.character
      end, 25), message .. " did not move the editor to the selected Definition")
      local current_buf = vim.api.nvim_get_current_buf()
      local destination = vim.api.nvim_buf_get_lines(current_buf, expected_line - 1, expected_line, false)[1] or ""
      helpers.assert_true(destination:find("@Definition", 1, true) ~= nil,
        message .. " must land on the Definition declaration")
    end

    local response, err = h.mcdev_command("mcdev.completion", {
      context = at("this."),
      trigger = { kind = "manual" },
    })
    helpers.assert_nil(err, "expression member completion failed: " .. vim.inspect(err))
    local candidates = vim.tbl_filter(function(item)
      return item.insertText == "counter"
    end, response.result.items or {})
    helpers.assert_eq(#candidates, 1, "reachable counter field must be offered exactly once: " .. vim.inspect(response))
    local item = completion.to_lsp_item(candidates[1])
    helpers.assert_not_nil(item.textEdit, "expression completion must contain its primary edit")
    helpers.assert_true(#(item.additionalTextEdits or {}) > 0, "expression completion must create its Definition")
    local edits = { item.textEdit }
    vim.list_extend(edits, item.additionalTextEdits)
    vim.lsp.util.apply_text_edits(edits, bufnr, "utf-16")
    local source = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
    helpers.assert_true(source:find('@Expression("this.counter")', 1, true) ~= nil, "member edit must complete the expression")
    helpers.assert_true(source:find("Lcom/example/target/SimpleTarget;counter:I", 1, true) ~= nil, "Definition must describe the actual field")
    helpers.assert_true(source:find("import com.llamalad7.mixinextras.expression.Definition;", 1, true) ~= nil, "Definition import must be inserted")
    helpers.assert_true(source:find("\n    @Definition(", 1, true) ~= nil,
      "Definition must retain the surrounding annotation indentation")
    helpers.assert_true(source:find('\n    @Expression("this.counter")', 1, true) ~= nil,
      "Definition insertion must preserve Expression indentation")
    vim.api.nvim_buf_call(bufnr, function() vim.cmd("silent update") end)
    h.compile_single_java_source(h.mixin_file)

    local definition, definition_err = h.mcdev_command("mcdev.definition", { context = at("this.coun") })
    helpers.assert_nil(definition_err, "expression identifier navigation failed")
    local locations = definition.result.locations or {}
    helpers.assert_eq(#locations, 1, "expression identifier must resolve its Definition")
    helpers.assert_eq(locations[1].documentUri, vim.uri_from_bufnr(bufnr), "identifier must navigate to this document's Definition")
    local destination = vim.api.nvim_buf_get_lines(bufnr, locations[1].range.start.line, locations[1].range.start.line + 1, false)[1]
    helpers.assert_true(destination:find("@Definition", 1, true) ~= nil, "identifier navigation must land on Definition")
    local navigable_locations = navigation_locations("this.coun")
    helpers.assert_eq(#navigable_locations, 1, "navigation adapter must retain the Definition location")
    helpers.assert_eq(navigable_locations[1].uri, vim.uri_from_bufnr(bufnr),
      "navigation adapter must preserve the Definition document")
    show_location(navigable_locations[1], "single Definition navigation")

    -- Complete the same member again with its Definition already present.
    -- Applying that item must reuse the binding, not duplicate annotations/imports.
    for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
      if line:find('@Expression("this.counter")', 1, true) then
        vim.api.nvim_buf_set_lines(bufnr, index - 1, index, false, { (line:gsub("this.counter", "this.cou", 1)) })
        break
      end
    end
    local reused, reused_err = h.mcdev_command("mcdev.completion", {
      context = at("this.cou"), trigger = { kind = "manual" },
    })
    helpers.assert_nil(reused_err, "repeat expression completion failed")
    local reused_items = vim.tbl_filter(function(candidate) return candidate.insertText == "counter" end, reused.result.items or {})
    helpers.assert_eq(#reused_items, 1, "repeat completion must retain the reachable member")
    local reused_item = completion.to_lsp_item(reused_items[1])
    helpers.assert_eq(#(reused_item.additionalTextEdits or {}), 0, "existing exact Definition must be reused")
    vim.lsp.util.apply_text_edits({ reused_item.textEdit }, bufnr, "utf-16")
    vim.api.nvim_buf_call(bufnr, function() vim.cmd("silent update") end)
    h.compile_single_java_source(h.mixin_file)
    for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
      if line:find("@Definition(", 1, true) then
        vim.api.nvim_buf_set_lines(bufnr, index, index, false, { line })
        break
      end
    end
    local repeated, repeated_err = h.mcdev_command("mcdev.definition", { context = at("this.coun") })
    helpers.assert_nil(repeated_err)
    local repeated_locations = repeated.result.locations or {}
    helpers.assert_eq(#repeated_locations, 2, "repeatable Definition IDs must expose both declaration locations")
    helpers.assert_true(repeated_locations[1].range.start.line ~= repeated_locations[2].range.start.line)
    local repeated_navigable = navigation_locations("this.coun")
    helpers.assert_eq(#repeated_navigable, 2, "navigation adapter must retain repeated Definition locations")
    helpers.assert_true(repeated_navigable[1].range.start.line ~= repeated_navigable[2].range.start.line,
      "repeated Definition locations must remain selectable")
    show_location(repeated_navigable[2], "second repeated Definition navigation")
    vim.api.nvim_buf_call(bufnr, function() vim.cmd("silent update") end)
    h.compile_single_java_source(h.mixin_file)
  end)

  h.with_buffer(h.mixin_file, "java", {
    "package com.example.mixin;",
    "import com.example.target.SimpleTarget;",
    "import com.llamalad7.mixinextras.expression.Expression;",
    "import com.llamalad7.mixinextras.expression.Definition;",
    "import com.llamalad7.mixinextras.sugar.Local;",
    "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
    "import org.spongepowered.asm.mixin.Mixin;",
    "import org.spongepowered.asm.mixin.injection.At;",
    "@Mixin(SimpleTarget.class)",
    "public abstract class MixinExtrasExample {",
    '    @Definition(id = "text", local = @Local(type = String.class, argsOnly = true, ordinal = 0))',
    '    @Definition(id = "primitive", type = int.class)',
    '    @Definition(id = "x", local = @Local(type = float.class, argsOnly = true, ordinal = 0))',
    '    @Expression("text.")',
    '    @ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))',
    "    private int modifyLength(int original) { return original; }",
    "}",
  }, function(bufnr)
    local context = h.build_context(bufnr, { 14, #'    @Expression("text.' })
    local response, err = h.mcdev_command("mcdev.completion", { context = context, trigger = { kind = "manual" } })
    helpers.assert_nil(err)
    local matches = vim.tbl_filter(function(item) return item.insertText == "length()" end, response.result.items or {})
    helpers.assert_eq(#matches, 1, "typed ordinal local must resolve its reachable String.length member: " .. vim.inspect(response))
    local item = completion.to_lsp_item(matches[1])
    local edits = { item.textEdit }
    vim.list_extend(edits, item.additionalTextEdits or {})
    vim.lsp.util.apply_text_edits(edits, bufnr, "utf-16")
    local source = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
    helpers.assert_true(source:find('@Expression("text.length()")', 1, true) ~= nil)
    helpers.assert_true(source:find("Ljava/lang/String;length()I", 1, true) ~= nil)
    vim.api.nvim_buf_call(bufnr, function() vim.cmd("silent update") end)
    h.compile_single_java_source(h.mixin_file)
    local diagnostics, diagnostic_err = h.mcdev_command("mcdev.diagnostics", {
      context = h.build_context(bufnr, { 10, 0 }),
    })
    helpers.assert_nil(diagnostic_err)
    helpers.assert_eq(#(diagnostics.result.diagnostics or {}), 0,
      "typed local expression and primitive Definition/Local types must resolve without diagnostics: " .. vim.inspect(diagnostics))
  end)

  -- A recognized MixinExtras handler must not hide another handler's context.
  -- All three annotations below previously disappeared only in mixed files.
  for _, handler in ipairs({
    { name = "WrapWithCondition v1", annotation = "com.llamalad7.mixinextras.injector.WrapWithCondition",
      signature = "private boolean guard(SimpleTarget instance) { return true; }" },
    { name = "WrapWithCondition v2", annotation = "com.llamalad7.mixinextras.injector.v2.WrapWithCondition",
      signature = "private boolean guard(SimpleTarget instance) { return true; }" },
    { name = "Inject", annotation = "org.spongepowered.asm.mixin.injection.Inject",
      signature = "private void guard(org.spongepowered.asm.mixin.injection.callback.CallbackInfo ci) {}" },
  }) do
    h.with_buffer(h.mixin_file, "java", {
      "package com.example.mixin;",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.expression.Expression;",
      "import com.llamalad7.mixinextras.expression.Definition;",
      "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      '    @Definition(id = "counter", field = "Lcom/example/target/SimpleTarget;counter:I")',
      '    @Expression("this.counter")',
      '    @ModifyExpressionValue(method = "compute()I", at = @At("MIXINEXTRAS:EXPRESSION"))',
      "    private int knownHandler(int original) { return original; }",
      '    @Expression("this.to")',
      '    @' .. handler.annotation .. '(method = "draw(Ljava/lang/String;FF)V", at = @At("MIXINEXTRAS:EXPRESSION"))',
      "    " .. handler.signature,
      "}",
    }, function(bufnr)
      local response, err = h.mcdev_command("mcdev.completion", {
        context = h.build_context(bufnr, { 14, #'    @Expression("this.to' }),
        trigger = { kind = "manual" },
      })
      helpers.assert_nil(err)
      local matches = vim.tbl_filter(function(item) return item.insertText == "touch()" end, response.result.items or {})
      helpers.assert_eq(#matches, 1, handler.name .. " must retain its own expression context: " .. vim.inspect(response))
      local item = completion.to_lsp_item(matches[1])
      local edits = { item.textEdit }
      vim.list_extend(edits, item.additionalTextEdits or {})
      vim.lsp.util.apply_text_edits(edits, bufnr, "utf-16")
      local source = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
      helpers.assert_true(source:find('@Expression("this.touch()")', 1, true) ~= nil)
      helpers.assert_true(source:find("Lcom/example/target/SimpleTarget;touch()V", 1, true) ~= nil)
      vim.api.nvim_buf_call(bufnr, function() vim.cmd("silent update") end)
      h.compile_single_java_source(h.mixin_file)
      h.log_step("mixed expression handler completion/edit/javac passed: " .. handler.name)
      if handler.name == "WrapWithCondition v2" then
        local valid_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
        local bad_line, bad_start, expression_line
        for index, line in ipairs(valid_lines) do
          if line:find("touch()V", 1, true) then
            local malformed = "    @com.llamalad7.mixinextras.expression.Definitions({ " .. vim.trim(line) .. ", 123 })"
            bad_line, bad_start = index, malformed:find("123", 1, true)
            vim.api.nvim_buf_set_lines(bufnr, index - 1, index, false, { malformed })
          elseif line:find('@Expression("this.touch()")', 1, true) then
            expression_line = index
            vim.api.nvim_buf_set_lines(bufnr, index - 1, index, false, { '    @Expression("this.to")' })
          end
        end
        helpers.assert_not_nil(bad_line)
        helpers.assert_not_nil(expression_line)
        local retained, retained_err = h.mcdev_command("mcdev.completion", {
          context = h.build_context(bufnr, { expression_line, #'    @Expression("this.to' }),
          trigger = { kind = "manual" },
        })
        helpers.assert_nil(retained_err)
        local retained_items = vim.tbl_filter(function(candidate) return candidate.insertText == "touch()" end,
          retained.result.items or {})
        helpers.assert_eq(#retained_items, 1, "malformed array element must not discard its valid Definition sibling")
        local retained_item = completion.to_lsp_item(retained_items[1])
        helpers.assert_eq(#(retained_item.additionalTextEdits or {}), 0, "valid sibling binding must be reused")
        vim.lsp.util.apply_text_edits({ retained_item.textEdit }, bufnr, "utf-16")
        local diagnostics, diagnostic_err = h.mcdev_command("mcdev.diagnostics", {
          context = h.build_context(bufnr, { bad_line, 0 }),
        })
        helpers.assert_nil(diagnostic_err)
        local issues = vim.tbl_filter(function(diagnostic) return diagnostic.code == "MIXINEXTRAS_INVALID_DEFINITION" end,
          diagnostics.result.diagnostics or {})
        helpers.assert_eq(#issues, 1, "only the malformed Definition array element must be diagnosed")
        helpers.assert_eq(issues[1].range.start.line, bad_line - 1)
        helpers.assert_eq(issues[1].range.start.character, bad_start - 1)
        helpers.assert_eq(issues[1].range["end"].line, bad_line - 1)
        helpers.assert_eq(issues[1].range["end"].character, bad_start + 2)
        vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, valid_lines)
        h.log_step("mixed handler malformed Definition sibling/reuse/exact diagnostic range passed")
      end
    end)
  end
end
