return function(h)
  local helpers = h.helpers
  local config = require("mcdev.config")
  local diagnostics = require("mcdev.diagnostics")

  local expected_descriptor =
    "(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/block/state/BlockState;"
    .. "Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;"
    .. "Lnet/minecraft/world/entity/player/Player;Lnet/minecraft/world/InteractionHand;"
    .. "Lnet/minecraft/world/phys/BlockHitResult;)Lnet/minecraft/world/ItemInteractionResult;"

  config.setup({ diagnostics = { enabled = true } })
  diagnostics.start()
  helpers.assert_eq(config.options.diagnostics.debounce_ms, 500)
  helpers.assert_eq(config.options.diagnostics.insert_mode, true)
  helpers.assert_eq(
    table.concat(config.options.diagnostics.events, ","),
    "TextChanged,TextChangedI,TextChangedP,InsertLeave,BufWritePost"
  )

  local function find_selector(lines, selector)
    for line_number, line in ipairs(lines) do
      local start = line:find(selector, 1, true)
      if start then
        return line_number, start - 1
      end
    end
    return nil, nil
  end

  local function diagnostics_for(bufnr)
    return vim.diagnostic.get(bufnr, { namespace = diagnostics.namespace })
  end

  local function has_code(bufnr, code)
    for _, diagnostic in ipairs(diagnostics_for(bufnr)) do
      if diagnostic.code == code then
        return true
      end
    end
    return false
  end

  local original_publish = diagnostics.publish
  local publish_count = 0
  diagnostics.publish = function(bufnr, values)
    publish_count = publish_count + 1
    return original_publish(bufnr, values)
  end

  local function jdt_request(method, params, timeout_ms)
    local result, request_error, done = nil, nil, false
    local started = h.client:request(method, params, function(err, response)
      request_error, result, done = err, response, true
    end)
    helpers.assert_true(started, method .. " request did not start")
    helpers.assert_true(
      vim.wait(timeout_ms or 60000, function()
        return done
      end, 100),
      method .. " timed out"
    )
    helpers.assert_nil(request_error, method .. " failed: " .. vim.inspect(request_error))
    return result
  end

  h.with_buffer(h.mixin_file, "java", nil, function(bufnr)
    local original_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local selector_line, selector_character = find_selector(original_lines, "useItemOn")
    helpers.assert_not_nil(selector_line, "useItemOn selector not found")

    local context = h.build_context(bufnr, { selector_line, selector_character })
    local info_result, info_err = h.mcdev_command("mcdev.info", { context = context }, 120000)
    helpers.assert_nil(info_err, "mcdev.info failed: " .. vim.inspect(info_err))
    helpers.assert_not_nil(info_result, "mcdev.info returned no payload")
    local definition_result, definition_err = h.mcdev_command(
      "mcdev.definition",
      { context = context },
      120000
    )
    helpers.assert_nil(definition_err, "mcdev.definition failed: " .. vim.inspect(definition_err))
    local locations = definition_result.result and definition_result.result.locations or {}
    local method_location = vim.tbl_filter(function(location)
      return location.metadata
        and location.metadata.kind == "method"
        and location.metadata.name == "useItemOn"
    end, locations)[1]
    helpers.assert_not_nil(
      method_location,
      "useItemOn definition was not returned: " .. vim.inspect(locations)
    )
    helpers.assert_eq(method_location.metadata.owner, "com/simibubi/create/content/kinetics/drill/DrillBlock")
    helpers.assert_eq(method_location.metadata.descriptor, expected_descriptor)
    helpers.assert_eq(method_location.resolution, "jdt")
    helpers.assert_true(
      method_location.documentUri:find("jdt://contents/create-1.21.1-6.0.10-280.jar/", 1, true) ~= nil,
      "definition must use the JDT classfile URI: " .. vim.inspect(method_location)
    )
    helpers.assert_true(
      method_location.documentUri:find("DrillBlock.java", 1, true) ~= nil,
      "definition must point at DrillBlock.java: " .. vim.inspect(method_location)
    )
    local attached_source = jdt_request("java/classFileContents", { uri = method_location.documentUri }, 120000)
    helpers.assert_true(type(attached_source) == "string", "JDT attached source must be text")
    helpers.assert_true(
      attached_source:find("useItemOn", 1, true) ~= nil,
      "JDT attached source must contain useItemOn"
    )
    local source_lines = vim.split(attached_source:gsub("\r\n", "\n"), "\n", { plain = true })
    local range_line = method_location.range and method_location.range.start and method_location.range.start.line
    helpers.assert_true(type(range_line) == "number", "definition must return a start range")
    helpers.assert_true(
      source_lines[range_line + 1] and source_lines[range_line + 1]:find("useItemOn", 1, true) ~= nil,
      "definition range must select useItemOn: " .. vim.inspect(method_location.range)
    )
    h.log_step("method definition resolved to " .. method_location.documentUri)

    local function set_method_name(name)
      local lines = vim.deepcopy(original_lines)
      for line_number, line in ipairs(lines) do
        if line:find("useItemOn", 1, true) then
          lines[line_number] = line:gsub("useItemOn", name, 1)
          break
        end
      end
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
    end

    local function trigger_text_changed()
      vim.api.nvim_exec_autocmds("TextChanged", { buffer = bufnr, modeline = false })
    end

    local function wait_for_request(previous_count)
      helpers.assert_true(
        vim.wait(10000, function()
          return diagnostics.request_count > previous_count
        end, 50),
        "diagnostics event did not issue a request"
      )
    end

    set_method_name("missingUseItemOn")
    local first_request_count = diagnostics.request_count
    trigger_text_changed()
    wait_for_request(first_request_count)
    local first_publish_count = publish_count
    helpers.assert_true(
      vim.wait(30000, function()
        return publish_count > first_publish_count and has_code(bufnr, "MIXIN_UNRESOLVED_INJECT_METHOD")
      end, 50),
      "unsaved wrong method should publish MIXIN_UNRESOLVED_INJECT_METHOD"
    )
    helpers.assert_true(vim.bo[bufnr].modified, "diagnostic check must remain unsaved")

    local fix_request_count = diagnostics.request_count
    local fix_publish_count = publish_count
    set_method_name("useItemOn")
    trigger_text_changed()
    wait_for_request(fix_request_count)
    helpers.assert_true(
      vim.wait(30000, function()
        return publish_count > fix_publish_count and not has_code(bufnr, "MIXIN_UNRESOLVED_INJECT_METHOD")
      end, 50),
      "unsaved method correction should clear the target diagnostic"
    )

    local burst_request_count = diagnostics.request_count
    local burst_publish_count = publish_count
    for _, method_name in ipairs({ "missingOne", "missingTwo", "missingThree", "useItemOn" }) do
      set_method_name(method_name)
      trigger_text_changed()
    end
    wait_for_request(burst_request_count)
    helpers.assert_eq(
      diagnostics.request_count,
      burst_request_count + 1,
      "rapid edits should coalesce into one latest diagnostics request"
    )
    helpers.assert_true(
      vim.wait(30000, function()
        return publish_count > burst_publish_count and not has_code(bufnr, "MIXIN_UNRESOLVED_INJECT_METHOD")
      end, 50),
      "rapid unsaved edits must leave the latest correct diagnostics"
    )
    helpers.assert_eq(publish_count, burst_publish_count + 1, "rapid edits should publish one latest response")
    local settled_request_count = diagnostics.request_count
    vim.wait(1000)
    helpers.assert_eq(
      diagnostics.request_count,
      settled_request_count,
      "debounced rapid edits must not issue another request after settling"
    )
    helpers.assert_true(
      not has_code(bufnr, "MIXIN_UNRESOLVED_INJECT_METHOD"),
      "rapid diagnostics must not restore the stale target error"
    )
    helpers.assert_true(vim.bo[bufnr].modified, "rapid diagnostic edits must not save the buffer")

    diagnostics.publish = original_publish
    diagnostics.stop()
    dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/jdt_action_aliases_e2e.lua")(h, bufnr)
  end)
end
