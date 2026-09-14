return function(h)
  local helpers = assert(h.helpers, "sugar E2E requires helpers")
  local with_buffer = assert(h.with_buffer, "sugar E2E requires with_buffer")
  local build_context = assert(h.build_context, "sugar E2E requires build_context")
  local mcdev_command = assert(h.mcdev_command, "sugar E2E requires mcdev_command")
  local compile_single_java_source = assert(
    h.compile_single_java_source,
    "sugar E2E requires compile_single_java_source"
  )
  local completion = require("mcdev.completion")
  local log_step = h.log_step or function() end

  local function append(lines, values)
    for _, value in ipairs(values) do
      lines[#lines + 1] = value
    end
  end

  local function java_source(extra_imports, handlers)
    local lines = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
    }
    append(lines, extra_imports)
    lines[#lines + 1] = ""
    lines[#lines + 1] = "@Mixin(SimpleTarget.class)"
    lines[#lines + 1] = "public abstract class MixinExtrasExample {"
    for index, handler in ipairs(handlers) do
      append(lines, handler)
      if index < #handlers then
        lines[#lines + 1] = ""
      end
    end
    lines[#lines + 1] = "}"
    return lines
  end

  local function find_marker(lines, marker)
    for line_number, line in ipairs(lines) do
      local start = line:find(marker, 1, true)
      if start then
        return line_number, start - 1
      end
    end
    return nil, nil
  end

  local function source_text(bufnr)
    return table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  end

  local function set_source(bufnr, lines)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  end

  local function apply_completion(bufnr, test_case)
    local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local line, character = find_marker(lines, test_case.completion.marker)
    helpers.assert_not_nil(line, test_case.name .. " completion marker not found")
    local response, err = mcdev_command("mcdev.completion", {
      context = build_context(bufnr, { line, character + #test_case.completion.marker }),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(err, test_case.name .. " completion failed: " .. vim.inspect(err))
    helpers.assert_not_nil(response, test_case.name .. " completion returned no response")

    local result = response.result or {}
    local items = result.items or {}
    for _, excluded in ipairs(test_case.completion.excluded_items or {}) do
      helpers.assert_eq(#vim.tbl_filter(function(item)
        return item.insertText == excluded
      end, items), 0, test_case.name .. " must exclude " .. excluded)
    end
    local matching = vim.tbl_filter(function(item)
      return item.insertText == test_case.completion.insert_text
    end, items)
    helpers.assert_true(
      #matching > 0,
      test_case.name
        .. " completion item missing: "
        .. test_case.completion.insert_text
        .. " (items="
        .. vim.inspect(items)
        .. ", debug="
        .. vim.inspect(result.debug or {})
        .. ")"
    )
    if test_case.completion.matching_count then
      helpers.assert_eq(
        #matching,
        test_case.completion.matching_count,
        test_case.name .. " completion item count"
      )
    end

    local item = completion.to_lsp_item(matching[1])
    helpers.assert_not_nil(item.textEdit, test_case.name .. " completion must provide a text edit")
    local edits = { item.textEdit }
    vim.list_extend(edits, item.additionalTextEdits or {})
    vim.lsp.util.apply_text_edits(edits, bufnr, "utf-16")

    local source = source_text(bufnr)
    for _, expected in ipairs(test_case.completion.expected_source or {}) do
      helpers.assert_true(
        source:find(expected, 1, true) ~= nil,
        test_case.name .. " completion edit missing expected source: " .. expected
      )
    end
    log_step("sugar completion applied " .. test_case.name)
  end

  local function diagnose(bufnr, test_case)
    local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local line, character = find_marker(lines, test_case.diagnostic_marker or "@Mixin")
    helpers.assert_not_nil(line, test_case.name .. " diagnostic marker not found")
    local response, err = mcdev_command("mcdev.diagnostics", {
      context = build_context(bufnr, { line, character }),
    })
    helpers.assert_nil(err, test_case.name .. " diagnostics failed: " .. vim.inspect(err))
    helpers.assert_not_nil(response, test_case.name .. " diagnostics returned no response")
    local diagnostics = (response.result and response.result.diagnostics) or {}
    local expected = test_case.expected_diagnostic
    if expected then
      local matches = vim.tbl_filter(function(diagnostic)
        return diagnostic.code == expected.code
      end, diagnostics)
      helpers.assert_eq(
        #matches,
        expected.count or 1,
        test_case.name .. " diagnostic count: " .. vim.inspect(diagnostics)
      )
      if expected.message_contains then
        helpers.assert_true(
          matches[1].message:find(expected.message_contains, 1, true) ~= nil,
          test_case.name .. " diagnostic message: " .. vim.inspect(matches[1])
        )
      end
    else
      helpers.assert_eq(
        #diagnostics,
        0,
        test_case.name .. " should be diagnostic-free: " .. vim.inspect(diagnostics)
      )
    end
  end

  local modify_length =
    '@ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))'

  local cases = {
    {
      name = "@Local ordinal with mutable LocalFloatRef",
      lines = java_source({
        "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
        "import com.llamalad7.mixinextras.sugar.Local;",
        "import com.llamalad7.mixinextras.sugar.ref.LocalFloatRef;",
      }, {
        {
          modify_length,
          "    private int mcdev$localOrdinal(int original, @Local(ordinal = ) LocalFloatRef localX) {",
          "        return original;",
          "    }",
        },
      }),
      completion = {
        marker = "@Local(ordinal = ",
        insert_text = "0",
        expected_source = { "@Local(ordinal = 0) LocalFloatRef localX" },
      },
      diagnostic_marker = "mcdev$localOrdinal",
    },
    {
      name = "@Local index",
      lines = java_source({
        "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
        "import com.llamalad7.mixinextras.sugar.Local;",
      }, {
        {
          modify_length,
          "    private int mcdev$localIndex(int original, @Local(index = ) String text) {",
          "        return original;",
          "    }",
        },
      }),
      completion = {
        marker = "@Local(index = ",
        insert_text = "1",
        matching_count = 1,
        expected_source = { "@Local(index = 1) String text" },
      },
      diagnostic_marker = "mcdev$localIndex",
    },
    {
      name = "@Local name with argsOnly",
      lines = java_source({
        "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
        "import com.llamalad7.mixinextras.sugar.Local;",
      }, {
        {
          modify_length,
          '    private int mcdev$localName(int original, @Local(argsOnly = true, name = "te") String text) {',
          "        return original;",
          "    }",
        },
      }),
      completion = {
        marker = '@Local(argsOnly = true, name = "te',
        insert_text = "text",
        matching_count = 1,
        expected_source = { '@Local(argsOnly = true, name = "text") String text' },
      },
      diagnostic_marker = "mcdev$localName",
    },
    {
      name = "@Share namespace and type alignment",
      lines = java_source({
        "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
        "import com.llamalad7.mixinextras.sugar.Share;",
        "import com.llamalad7.mixinextras.sugar.ref.LocalFloatRef;",
        "import com.llamalad7.mixinextras.sugar.ref.LocalIntRef;",
      }, {
        {
          modify_length,
          '    private int mcdev$shareAligned(int original, @Share(value = "speed", namespace = "shared") LocalIntRef speed) {',
          "        return original;",
          "    }",
        },
        {
          modify_length,
          '    private int mcdev$shareOtherNamespace(int original, @Share(value = "spOtherNamespace", namespace = "other") LocalIntRef otherNamespace) {',
          "        return original;",
          "    }",
        },
        {
          modify_length,
          '    private int mcdev$shareOtherType(int original, @Share(value = "spOtherType", namespace = "shared") LocalFloatRef otherType) {',
          "        return original;",
          "    }",
        },
        {
          modify_length,
          '    private int mcdev$shareCurrent(int original, @Share(value = "sp", namespace = "shared") LocalIntRef partial) {',
          "        return original;",
          "    }",
        },
      }),
      completion = {
        marker = 'private int mcdev$shareCurrent(int original, @Share(value = "sp',
        insert_text = "speed",
        excluded_items = { "spOtherNamespace", "spOtherType" },
        matching_count = 1,
        expected_source = {
          '@Share(value = "speed", namespace = "shared") LocalIntRef partial',
        },
      },
      diagnostic_marker = "mcdev$shareCurrent",
    },
    {
      name = "@Cancellable void target CallbackInfo",
      lines = java_source({
        "import com.llamalad7.mixinextras.sugar.Cancellable;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
      }, {
        {
          '@Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))',
          "    private void mcdev$cancellableVoid(CallbackInfo ci, @Cancellable CallbackInfo cancellable) {}",
        },
      }),
      diagnostic_marker = "@Cancellable",
    },
    {
      name = "@Cancellable void target rejects CallbackInfoReturnable",
      lines = java_source({
        "import com.llamalad7.mixinextras.sugar.Cancellable;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;",
      }, {
        {
          '@Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))',
          "    private void mcdev$badCancellableVoid(CallbackInfo ci, @Cancellable CallbackInfoReturnable<Integer> cancellable) {}",
        },
      }),
      diagnostic_marker = "@Cancellable",
      expected_diagnostic = {
        code = "MIXINEXTRAS_HANDLER_SIGNATURE_MISMATCH",
        message_contains = "Cancellable parameter type should be",
      },
    },
    {
      name = "@Cancellable does not replace the Inject callback",
      lines = java_source({
        "import com.llamalad7.mixinextras.sugar.Cancellable;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
      }, {
        {
          '@Inject(method = "draw(Ljava/lang/String;FF)V", at = @At("HEAD"))',
          "    private void mcdev$missingCallback(@Cancellable CallbackInfo cancellable) {}",
        },
      }),
      diagnostic_marker = "@Cancellable",
      expected_diagnostic = {
        code = "MIXINEXTRAS_HANDLER_SIGNATURE_MISMATCH",
        message_contains = "Inject handler must",
      },
    },
    {
      name = "@Cancellable non-void target CallbackInfoReturnable",
      lines = java_source({
        "import com.llamalad7.mixinextras.sugar.Cancellable;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;",
      }, {
        {
          '@Inject(method = "compute()I", at = @At("RETURN"))',
          "    private void mcdev$cancellableNonVoid(CallbackInfoReturnable<Integer> cir, @Cancellable CallbackInfoReturnable<Integer> cancellable) {}",
        },
      }),
      diagnostic_marker = "@Cancellable",
    },
    {
      name = "@Cancellable non-void target rejects CallbackInfo",
      lines = java_source({
        "import com.llamalad7.mixinextras.sugar.Cancellable;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;",
      }, {
        {
          '@Inject(method = "compute()I", at = @At("RETURN"))',
          "    private void mcdev$badCancellableNonVoid(CallbackInfoReturnable<Integer> cir, @Cancellable CallbackInfo cancellable) {}",
        },
      }),
      diagnostic_marker = "@Cancellable",
      expected_diagnostic = {
        code = "MIXINEXTRAS_HANDLER_SIGNATURE_MISMATCH",
        message_contains = "Cancellable parameter type should be",
      },
    },
  }

  with_buffer(h.mixin_file, "java", cases[1].lines, function(bufnr)
    for _, test_case in ipairs(cases) do
      set_source(bufnr, test_case.lines)
      if test_case.completion then
        apply_completion(bufnr, test_case)
      end
      vim.api.nvim_buf_call(bufnr, function()
        vim.cmd("silent update")
      end)
      compile_single_java_source(h.mixin_file)
      diagnose(bufnr, test_case)
      log_step("sugar case passed " .. test_case.name)
    end
  end)
end
