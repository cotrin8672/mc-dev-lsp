return function(h)
  local helpers = h.helpers
  local completion = require("mcdev.completion")

  local target_prefix = "Lnet/minecraft/world/level/block/Block;setPl"
  local expected_target =
    "Lnet/minecraft/world/level/block/Block;setPlacedBy("
    .. "Lnet/minecraft/world/level/Level;"
    .. "Lnet/minecraft/core/BlockPos;"
    .. "Lnet/minecraft/world/level/block/state/BlockState;"
    .. "Lnet/minecraft/world/entity/LivingEntity;"
    .. "Lnet/minecraft/world/item/ItemStack;)V"

  local function count_occurrences(text, needle)
    local count = 0
    local offset = 1
    while true do
      local start = text:find(needle, offset, true)
      if not start then
        return count
      end
      count = count + 1
      offset = start + #needle
    end
  end

  local function source_text(bufnr)
    return table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  end

  local function find_fragment(lines, fragment)
    for line_number, line in ipairs(lines) do
      local start = line:find(fragment, 1, true)
      if start then
        return line_number, start - 1
      end
    end
    return nil, nil
  end

  local function target_line(lines)
    local line_number = find_fragment(lines, "target = ")
    helpers.assert_not_nil(line_number, "PlacementOffset @At target line not found")
    return line_number
  end

  local function split_target_source(original_lines)
    local lines = vim.deepcopy(original_lines)
    local first_line = target_line(lines)
    helpers.assert_true(
      lines[first_line]:find("setPlacedBy(", 1, true) ~= nil,
      "PlacementOffset source must contain the original setPlacedBy target"
    )
    lines[first_line] = lines[first_line]:gsub("setPlacedBy%(", "setPl", 1)
    local second_line = first_line + 1
    local opening_quote = lines[second_line]:find('"', 1, true)
    helpers.assert_not_nil(opening_quote, "concatenated target continuation not found")
    lines[second_line] = lines[second_line]:sub(1, opening_quote)
      .. "acedBy("
      .. lines[second_line]:sub(opening_quote + 1)
    return lines, first_line, second_line
  end

  local function single_target_source(original_lines, target)
    local lines = vim.deepcopy(original_lines)
    local first_line = target_line(lines)
    local indentation = lines[first_line]:match("^(%s*)") or ""
    lines[first_line] = indentation .. 'target = "' .. target .. '",'
    local remap_line = first_line + 1
    while remap_line <= #lines and not lines[remap_line]:find("remap = true", 1, true) do
      remap_line = remap_line + 1
    end
    helpers.assert_true(remap_line <= #lines, "PlacementOffset nested @At remap flag not found")
    for line_number = remap_line - 1, first_line + 1, -1 do
      table.remove(lines, line_number)
    end
    return lines, first_line
  end

  local function diagnostic_failure_summary(diagnostics)
    local summary = {}
    for index = 1, math.min(#diagnostics, 3) do
      local diagnostic = diagnostics[index]
      summary[#summary + 1] = string.format(
        "%s: %s metadata=%s",
        tostring(diagnostic.code),
        tostring(diagnostic.message),
        vim.inspect(diagnostic.metadata or {})
      )
    end
    return table.concat(summary, " | ")
  end

  local function assert_final_target(bufnr, label)
    local source = source_text(bufnr)
    local target_attribute = 'target = "' .. expected_target .. '",'
    local target_attribute_start = source:find('target = "', 1, true)
    helpers.assert_not_nil(target_attribute_start, label .. " target attribute not found")
    local target_value_start = target_attribute_start + #'target = "'
    local target_value_end = source:find('"', target_value_start, true)
    helpers.assert_not_nil(target_value_end, label .. " target attribute is not closed")
    local target_value = source:sub(target_value_start, target_value_end - 1)
    helpers.assert_eq(target_value, expected_target, label .. " must contain one full target")
    helpers.assert_eq(count_occurrences(target_value, expected_target), 1, label .. " must not duplicate the target")
    helpers.assert_eq(count_occurrences(target_value, "acedBy("), 1, label .. " must not retain the old concatenated suffix")
    helpers.assert_eq(count_occurrences(source, target_attribute), 1, label .. " target attribute must be unique")
    helpers.assert_true(
      source:sub(target_value_end + 1):match("^,%s*remap%s*=%s*true") ~= nil,
      label .. " target must be immediately followed by remap=true"
    )
    helpers.assert_eq(count_occurrences(source, "remap = true"), 1, label .. " must preserve remap=true")
  end

  local function complete_at(bufnr, position, label)
    local response, err = h.mcdev_command("mcdev.completion", {
      context = h.build_context(bufnr, position),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "smart",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    }, 120000)
    helpers.assert_nil(err, label .. " completion failed: " .. vim.inspect(err))
    helpers.assert_not_nil(response, label .. " completion returned no response")
    local items = response.result and response.result.items or {}
    local matches = vim.tbl_filter(function(item)
      return item.metadata
        and item.metadata.source == "mixin.atTarget"
        and item.metadata.owner == "net/minecraft/world/level/block/Block"
        and item.metadata.name == "setPlacedBy"
    end, items)
    helpers.assert_eq(#matches, 1, label .. " must select one setPlacedBy item: " .. vim.inspect(items))
    helpers.assert_eq(matches[1].insertText, expected_target, label .. " must return the full remapped target")
    local item = completion.to_lsp_item(matches[1])
    helpers.assert_not_nil(item.textEdit, label .. " must return a textEdit")
    vim.lsp.util.apply_text_edits({ item.textEdit }, bufnr, "utf-16")
  end

  local native_blink_cmp

  local function native_blink_complete_at(bufnr, lines, position, label)
    local blink_path = vim.env.MCDEV_E2E_BLINK_RTP
    helpers.assert_not_nil(blink_path, "MCDEV_E2E_BLINK_RTP must point to the installed blink.cmp checkout")
    local blink_lib_path = vim.env.MCDEV_E2E_BLINK_LIB_RTP
    helpers.assert_not_nil(blink_lib_path, "MCDEV_E2E_BLINK_LIB_RTP must point to the installed blink.lib checkout")
    if not native_blink_cmp then
      vim.opt.runtimepath:append(blink_lib_path)
      vim.opt.runtimepath:append(blink_path)
      native_blink_cmp = require("blink.cmp")
      native_blink_cmp.setup({
        enabled = function() return true end,
        keymap = { preset = "enter" },
        completion = { list = { selection = { preselect = false, auto_insert = false } } },
        sources = {
          default = { "mcdev" },
          providers = {
            mcdev = { module = "mcdev.blink", timeout_ms = 0 },
          },
        },
      })
      helpers.assert_true(vim.wait(30000, function()
        local trigger = package.loaded["blink.cmp.completion.trigger"]
        return trigger ~= nil and trigger.buffer_events ~= nil
      end, 25), label .. " Blink setup did not finish")
    end
    local blink_cmp = native_blink_cmp

    local readonly_before_pump = vim.bo[bufnr].readonly
    h.log_step(label .. " stage=start mode=" .. vim.api.nvim_get_mode().mode
      .. " readonly=" .. tostring(readonly_before_pump))
    vim.bo[bufnr].readonly = false
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
    vim.api.nvim_win_set_cursor(0, position)
    local function input(keys)
      vim.api.nvim_input(keys)
    end
    local state = {
      deadline = vim.uv.now() + 30000,
      accepted = false,
      error = nil,
      terminal = false,
    }
    local function fail(message)
      if state.terminal then return end
      state.error = state.error or tostring(message)
      state.terminal = true
      pcall(input, "<Esc><Esc>")
    end
    local function schedule(callback)
      vim.defer_fn(function()
        if state.terminal then return end
        if vim.uv.now() >= state.deadline then
          fail(label .. " native input timed out")
          return
        end
        local ok, err = pcall(callback)
        if not ok then
          fail(label .. " native callback failed: " .. tostring(err))
        end
      end, 25)
    end

    local function wait_for_accept()
      if not blink_cmp.is_menu_visible() and source_text(bufnr):find(expected_target, 1, true) ~= nil then
        if vim.api.nvim_get_mode().mode:sub(1, 1) ~= "i" then
          fail(label .. " native Enter left insert mode before cursor check")
          return
        end
        local target_row, target_start = find_fragment(
          vim.api.nvim_buf_get_lines(bufnr, 0, -1, false),
          'target = "' .. expected_target
        )
        if not target_row then
          fail(label .. " target row not found after native accept")
          return
        end
        local insert_column = target_start + #'target = "' + #expected_target
        local cursor = vim.api.nvim_win_get_cursor(0)
        if cursor[1] ~= target_row or cursor[2] ~= insert_column then
          fail(label .. " insert-mode cursor mismatch: " .. vim.inspect(cursor))
          return
        end
        state.target_row = target_row
        state.insert_column = insert_column
        input("<Esc>")
        state.accepted = true
        h.log_step(label .. " stage=accepted")
        state.terminal = true
      else
        schedule(wait_for_accept)
      end
    end

    local function wait_for_menu()
      local context = blink_cmp.get_context()
      local cursor = context and context.cursor or {}
      if blink_cmp.is_menu_visible()
          and context and context.bufnr == bufnr
          and cursor[1] == position[1] and cursor[2] == position[2] then
        local target_index
        local items = blink_cmp.get_items() or {}
        for index, item in ipairs(items) do
          local data = item.data or {}
          if data.source == "mixin.atTarget" and data.name == "setPlacedBy" then
            target_index = index
            break
          end
        end
        if target_index then
          h.log_step(label .. " stage=menu")
          local target_item = items[target_index]
          if not target_item.textEdit or not target_item.textEdit.range
              or target_item.textEdit.range.start.line ~= target_item.textEdit.range["end"].line then
            fail(label .. " must expose a single-line primary edit")
            return
          end
          local selected, select_error = pcall(function()
            require("blink.cmp.completion.list").select(target_index, { is_explicit_selection = true })
          end)
          if not selected then
            fail(label .. " selected item failed: " .. tostring(select_error))
            return
          end
          local selected_item = blink_cmp.get_selected_item()
          if not selected_item or not selected_item.data or selected_item.data.name ~= "setPlacedBy" then
            fail(label .. " selected wrong item")
            return
          end
          input("<CR>")
          schedule(wait_for_accept)
          return
        end
      end
      schedule(wait_for_menu)
    end

    local function start_completion()
      if vim.api.nvim_get_mode().mode:sub(1, 1) ~= "i" then
        fail(label .. " did not enter insert mode")
      else
        input("<C-Space>")
        schedule(wait_for_menu)
      end
    end

    schedule(start_completion)
    vim.defer_fn(function()
      if not state.terminal then
        fail(label .. " native input timed out")
        h.log_step(label .. " stage=watchdog")
      end
    end, 30000)
    vim.api.nvim_feedkeys("i", "nx!", false)

    helpers.assert_true(state.accepted, state.error or (label .. " native input did not finish"))
    helpers.assert_nil(state.error, state.error)
    helpers.assert_eq(vim.api.nvim_get_mode().mode, "n", label .. " must leave insert mode")
    assert_final_target(bufnr, label)

    local target_row, target_start = find_fragment(
      vim.api.nvim_buf_get_lines(bufnr, 0, -1, false),
      'target = "' .. expected_target
    )
    helpers.assert_not_nil(target_row, label .. " target row not found after native accept")
    local target_line = vim.api.nvim_buf_get_lines(bufnr, target_row - 1, target_row, false)[1]
    local insert_column = target_start + #'target = "' + #expected_target
    local normal_cursor = vim.api.nvim_win_get_cursor(0)
    helpers.assert_eq(normal_cursor[1], target_row, label .. " normal-mode cursor row")
    helpers.assert_eq(normal_cursor[2], insert_column - 1, label .. " normal-mode cursor column")
    helpers.assert_true(#target_line > insert_column, label .. " target line must retain its closing quote")
  end

  local function assert_clean_diagnostics(bufnr, label)
    local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local line, character = find_fragment(lines, "setPlacedBy(")
    helpers.assert_not_nil(line, label .. " target marker not found for diagnostics")
    local response, err = h.mcdev_command("mcdev.diagnostics", {
      context = h.build_context(bufnr, { line, character }),
    }, 120000)
    helpers.assert_nil(err, label .. " diagnostics failed: " .. vim.inspect(err))
    local diagnostics = response.result and response.result.diagnostics or {}
    local errors = vim.tbl_filter(function(diagnostic)
      return tostring(diagnostic.severity):lower() == "error"
    end, diagnostics)
    helpers.assert_eq(
      #errors,
      0,
      label .. " must not report mcdev errors: " .. diagnostic_failure_summary(errors)
    )
  end

  h.with_buffer(h.placement_mixin_file, "java", nil, function(bufnr)
    local original_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)

    helpers.assert_not_nil(find_fragment(original_lines, "setPlacedBy("), "unchanged PlacementOffset target not found")
    assert_clean_diagnostics(bufnr, "unchanged LF source")

    local crlf_lines = vim.tbl_map(function(line)
      return line .. "\r"
    end, original_lines)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, crlf_lines)
    assert_clean_diagnostics(bufnr, "unchanged CRLF source")
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, original_lines)

    local single_lines = single_target_source(original_lines, expected_target)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, single_lines)
    assert_clean_diagnostics(bufnr, "single full descriptor")

    local standalone_lines, standalone_line = single_target_source(original_lines, target_prefix)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, standalone_lines)
    complete_at(bufnr, { standalone_line, standalone_lines[standalone_line]:find(target_prefix, 1, true) - 1 + #target_prefix }, "standalone target")
    assert_final_target(bufnr, "standalone target")
    assert_clean_diagnostics(bufnr, "standalone target")

    local split_lines, first_line, second_line = split_target_source(original_lines)
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, split_lines)
    local first_character = split_lines[first_line]:find(target_prefix, 1, true) - 1 + #target_prefix
    complete_at(bufnr, { first_line, first_character }, "concatenated target first segment")
    assert_final_target(bufnr, "concatenated target first segment")
    assert_clean_diagnostics(bufnr, "concatenated target first segment")

    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, split_lines)
    local later_character = split_lines[second_line]:find("acedBy", 1, true) - 1 + #"acedBy"
    complete_at(bufnr, { second_line, later_character }, "concatenated target later segment")
    assert_final_target(bufnr, "concatenated target later segment")
    assert_clean_diagnostics(bufnr, "concatenated target later segment")

    native_blink_complete_at(
      bufnr,
      split_lines,
      { first_line, first_character },
      "concatenated target first literal native Blink completion"
    )
    assert_clean_diagnostics(bufnr, "concatenated target first literal native Blink completion")

    native_blink_complete_at(
      bufnr,
      split_lines,
      { second_line, split_lines[second_line]:find("acedBy", 1, true) - 1 + #"acedBy" },
      "concatenated target native Blink completion"
    )
    assert_clean_diagnostics(bufnr, "concatenated target native Blink completion")

    local original_target_line = target_line(original_lines)
    native_blink_complete_at(
      bufnr,
      original_lines,
      { original_target_line + 2, 43 },
      "original BlockPos literal native Blink completion"
    )
    assert_clean_diagnostics(bufnr, "original BlockPos literal native Blink completion")
  end)
end
