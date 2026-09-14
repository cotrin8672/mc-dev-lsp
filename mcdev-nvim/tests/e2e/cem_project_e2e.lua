return function(h)
  local completion = require('mcdev.completion')
  local disk_source = table.concat(vim.fn.readfile(h.mixin_file),'\n')
  h.with_buffer(h.mixin_file, 'java', nil, function(bufnr)
    local original = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    -- Use the real project's Minecraft/NeoForge classes. These unfinished editor
    -- buffers are never written to the user's project.
    local function row_of_exact(lines, exact, description)
      for row, line in ipairs(lines) do
        if line == exact then return row end
      end
      h.helpers.assert_true(false, 'missing fixture line for ' .. description .. ': ' .. exact)
    end

    local member_reference_path = vim.env.MCDEV_E2E_MEMBER_REFERENCE
    local member_reference
    local member_reference_logged = false
    if not member_reference_path or member_reference_path == '' then
      h.log_step('CEM exact member comparison not configured (MCDEV_E2E_MEMBER_REFERENCE unset)')
      member_reference_logged = true
    end

    local function trim(value)
      return (value:match('^%s*(.-)%s*$'))
    end

    local function normalize_owner(owner)
      return (owner or ''):gsub('%.', '/')
    end

    local function sorted_keys(values)
      local keys = {}
      for key in pairs(values) do keys[#keys + 1] = key end
      table.sort(keys)
      return keys
    end

    local function compact_keys(keys, owner)
      local shown = {}
      local prefix = owner .. '|'
      for index, key in ipairs(keys) do
        if index > 40 then
          shown[#shown + 1] = '...'
          break
        end
        shown[#shown + 1] = key:sub(1, #prefix) == prefix and key:sub(#prefix + 1) or key
      end
      return table.concat(shown, ',')
    end

    local function load_member_reference()
      if member_reference ~= nil then return member_reference end
      if not member_reference_path or member_reference_path == '' then
        member_reference = false
        if not member_reference_logged then
          h.log_step('CEM exact member comparison not configured (MCDEV_E2E_MEMBER_REFERENCE unset)')
          member_reference_logged = true
        end
        return nil
      end

      h.helpers.assert_true(vim.fn.filereadable(member_reference_path) == 1,
        'MCDEV_E2E_MEMBER_REFERENCE is not readable: ' .. member_reference_path)
      local by_owner = {}
      local duplicate_lines = {}
      for line_number, line in ipairs(vim.fn.readfile(member_reference_path)) do
        if line_number == 1 then line = line:gsub('^\239\187\191', '') end
        if line ~= '' and not line:match('^%s*#') then
          local raw_owner, raw_name, raw_descriptor = line:match('^([^|]+)|([^|]+)|(.+)$')
          h.helpers.assert_not_nil(raw_owner, 'invalid member reference line ' .. tostring(line_number))
          local owner = normalize_owner(trim(raw_owner))
          local name = trim(raw_name)
          local descriptor = trim(raw_descriptor)
          local owner_set = by_owner[owner] or {}
          local key = owner .. '|' .. name .. '|' .. descriptor
          if owner_set[key] then
            duplicate_lines[#duplicate_lines + 1] = key
          end
          owner_set[key] = true
          by_owner[owner] = owner_set
        end
      end
      member_reference = {by_owner = by_owner, duplicate_lines = duplicate_lines}
      return member_reference
    end

    local function compare_member_reference(owner, result)
      local reference = load_member_reference()
      if not reference then return end
      local owner_internal = normalize_owner('net/minecraft/world/item/' .. owner)
      local expected = reference.by_owner[owner_internal] or {}
      local actual = {}
      local actual_duplicates = {}
      for _, item in ipairs(result.items or {}) do
        local data = item.data or {}
        local item_owner = normalize_owner(data.owner)
        if item_owner == owner_internal then
          local key = item_owner .. '|' .. tostring(data.name) .. '|' .. tostring(data.descriptor)
          if actual[key] then
            actual_duplicates[#actual_duplicates + 1] = key
          end
          actual[key] = true
        end
      end
      local missing = {}
      for key in pairs(expected) do
        if not actual[key] then missing[#missing + 1] = key end
      end
      local unexpected = {}
      for key in pairs(actual) do
        if not expected[key] then unexpected[#unexpected + 1] = key end
      end
      table.sort(missing)
      table.sort(unexpected)
      local expected_duplicates = reference.duplicate_lines
      if #missing > 0 or #unexpected > 0 or #actual_duplicates > 0 or #expected_duplicates > 0 then
        h.log_step('CEM exact member comparison FAILED owner=' .. owner ..
          ' expected_count=' .. tostring(#sorted_keys(expected)) ..
          ' actual_count=' .. tostring(#sorted_keys(actual)) ..
          ' missing=' .. compact_keys(missing, owner_internal) ..
          ' unexpected=' .. compact_keys(unexpected, owner_internal) ..
          ' duplicate_actual=' .. compact_keys(actual_duplicates, owner_internal) ..
          ' duplicate_reference=' .. compact_keys(expected_duplicates, owner_internal))
        h.helpers.assert_true(false, 'CEM ' .. owner .. ' exact member reference mismatch')
      end
      h.log_step('CEM exact member comparison owner=' .. owner ..
        ' count=' .. tostring(#sorted_keys(expected)))
    end

    local function complete(owner, prefix)
      local lines = {
        'package io.github.cotrin8672.cem.mixin;',
        'import net.minecraft.world.item.' .. owner .. ';',
        'import org.spongepowered.asm.mixin.Mixin;',
        'import org.spongepowered.asm.mixin.injection.Inject;',
        '@Mixin(' .. owner .. '.class)',
        'public class ItemMixin {',
        '    @Inject(method = "' .. prefix .. '")',
        '}',
      }
      local inject_row = row_of_exact(lines, '    @Inject(method = "' .. prefix .. '")', 'CEM selector')
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
      local result
      local cancel = completion.complete(function(value)
        if not value.isProvisional then result = value end
      end, bufnr, {inject_row, #'    @Inject(method = "' + #prefix}, {source='real-cem',stream=true})
      local finished = vim.wait(120000, function() return result ~= nil end, 100)
      if not finished then cancel() end
      h.helpers.assert_true(finished, 'CEM completion did not finalize within 120s')
      h.helpers.assert_nil(completion.last_error)
      h.helpers.assert_nil(completion.last_project_transport_error)
      h.helpers.assert_eq(completion.last_debug.owner, 'net/minecraft/world/item/' .. owner)
      local by_name = {}
      for _,item in ipairs(result.items or {}) do
        h.helpers.assert_eq(item.data.owner, 'net/minecraft/world/item/' .. owner)
        by_name[item.data.name] = item
      end
      h.helpers.assert_nil(by_name.world)
      h.helpers.assert_nil(by_name.setReturnValue)
      if prefix == '' then compare_member_reference(owner, result) end
      h.log_step('CEM ' .. owner .. ':' .. prefix .. ' final candidates=' .. #(result.items or {}))
      return by_name, lines, inject_row
    end

    local last_expected_completion_position

    local function log_completion_diagnostic(result, description, position)
      local items = result and result.items or {}
      local names = {}
      for index, item in ipairs(items) do
        if index > 80 then
          names[#names + 1] = '...'
          break
        end
        local data = item.data or {}
        local name = data.name or item.label or '<unnamed>'
        local kind = data.kind or item.kind or '?'
        names[#names + 1] = tostring(name) .. '/' .. tostring(kind)
      end
      local debug_values = {}
      for _, key in ipairs({
        'completionContextKind', 'owner', 'partialValue', 'kind',
        'methodName', 'methodDescriptor', 'candidateCountBeforeFilter',
        'candidateCountAfterFilter', 'zeroItemReason', 'semanticContextFound',
        'semanticTargetCount', 'semanticMemberCount', 'fallbackAnnotationContextUsed',
        'fallbackAnnotationContextReason', 'parseSource', 'parseConfidence',
      }) do
        if completion.last_debug and completion.last_debug[key] ~= nil then
          debug_values[key] = completion.last_debug[key]
        end
      end
      local debug = vim.inspect(debug_values):gsub('%s+', ' ')
      local cursor = position and (tostring(position[1]) .. ':' .. tostring(position[2])) or '<unknown>'
      h.log_step(description .. ' expected_cursor=' .. cursor ..
        ' count=' .. tostring(#items) .. ' items=[' .. table.concat(names, ',') ..
        '] last_debug=' .. debug)
    end

    local function complete_lines(lines, position, description)
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
      last_expected_completion_position = position
      local result
      local cancel = completion.complete(function(value)
        if not value.isProvisional then result = value end
      end, bufnr, position, {source='real-cem-shadow',stream=true})
      local finished = vim.wait(120000, function() return result ~= nil end, 100)
      if not finished then cancel() end
      h.helpers.assert_true(finished, description .. ' did not finalize within 120s')
      h.helpers.assert_nil(completion.last_error, description .. ' returned an error')
      h.helpers.assert_nil(completion.last_project_transport_error, description .. ' project transport failed')
      return result
    end

    local function position_after(line, marker, description)
      local start = line:find(marker, 1, true)
      h.helpers.assert_not_nil(start, 'missing caret marker for ' .. description)
      return start - 1 + #marker
    end

    local function exact_item(result, name, description)
      local match
      for _, item in ipairs(result.items or {}) do
        if item.data and item.data.name == name then
          h.helpers.assert_nil(match, description .. ' returned duplicate ' .. name)
          match = item
        end
      end
      if not match then
        log_completion_diagnostic(result, description .. ' missing ' .. name,
          last_expected_completion_position)
      end
      h.helpers.assert_not_nil(match, description .. ' missing ' .. name)
      return match
    end

    local function assert_no_item(result, name, description)
      for _, item in ipairs(result.items or {}) do
        h.helpers.assert_true(not (item.data and item.data.name == name),
          description .. ' returned opposite-kind member ' .. name)
      end
    end

    local items = complete('Item', 'is')
    h.helpers.assert_not_nil(items.isEnchantable)
    h.helpers.assert_nil(items.getEnchantmentValue)
    -- Deleting the prefix must expand the final result, including methods that
    -- never appeared in the narrow response.
    local expanded, lines, expanded_inject_row = complete('Item', '')
    for _,name in ipairs({'isEnchantable','getEnchantmentValue','getDefaultInstance','use','<init>'}) do
      h.helpers.assert_not_nil(expanded[name], 'missing Item.' .. name)
    end
    local chosen = expanded.getEnchantmentValue
    h.helpers.assert_not_nil(chosen.textEdit)
    vim.lsp.util.apply_text_edits({chosen.textEdit}, bufnr, 'utf-16')
    lines[expanded_inject_row] = '    @Inject(method = "getEnchantmentValue")'
    h.helpers.assert_eq(table.concat(vim.api.nvim_buf_get_lines(bufnr,0,-1,false),'\n'), table.concat(lines,'\n'))
    local block, block_lines, block_inject_row = complete('BlockItem', '')
    for _,name in ipairs({'getBlock','getPlacementState','canPlace','<init>'}) do
      h.helpers.assert_not_nil(block[name], 'missing BlockItem.' .. name)
    end
    h.helpers.assert_nil(block.isEnchantable, 'inherited methods are not injection targets declared in BlockItem')

    -- A caret inside an existing @Shadow declaration must replace the whole
    -- member token when the target name is applied.  The deliberately wrong
    -- suffixes make a prefix-only edit observable (`blockXock` would expose
    -- the regression immediately).
    local shadow_lines = {
      'package io.github.cotrin8672.cem.mixin;',
      'import net.minecraft.world.item.BlockItem;',
      'import net.minecraft.world.level.block.Block;',
      'import org.spongepowered.asm.mixin.Mixin;',
      'import org.spongepowered.asm.mixin.Shadow;',
      '@Mixin(BlockItem.class)',
      'public abstract class BlockItemMixin {',
      '    @Shadow private Block blXock;',
      '    @Shadow public abstract Block getBlXock();',
      '}',
    }
    local shadow_field_line = '    @Shadow private Block blXock;'
    local shadow_method_line = '    @Shadow public abstract Block getBlXock();'
    local shadow_field_row = row_of_exact(shadow_lines, shadow_field_line, 'BlockItem @Shadow field')
    local shadow_method_row = row_of_exact(shadow_lines, shadow_method_line, 'BlockItem @Shadow method')
    local shadow_field_column = position_after(shadow_field_line, 'bl', 'BlockItem @Shadow field')
    local shadow_method_column = position_after(shadow_method_line, 'getBl', 'BlockItem @Shadow method')

    local shadow_field_result = complete_lines(
      shadow_lines, {shadow_field_row, shadow_field_column}, 'BlockItem @Shadow field completion')
    local shadow_field_item = exact_item(shadow_field_result, 'block', 'BlockItem @Shadow field completion')
    h.helpers.assert_eq(shadow_field_item.insertText, 'block')
    h.helpers.assert_eq(shadow_field_item.data.source, 'mixin.shadow')
    h.helpers.assert_not_nil(shadow_field_item.textEdit, 'BlockItem @Shadow field must provide a text edit')
    vim.lsp.util.apply_text_edits({shadow_field_item.textEdit}, bufnr, 'utf-16')
    local shadow_after_field = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    h.helpers.assert_eq(shadow_after_field[shadow_field_row], '    @Shadow private Block block;')

    local shadow_field_all = complete_lines(
      shadow_after_field, {shadow_field_row, shadow_field_column - #'bl'},
      'BlockItem @Shadow field kind filter')
    exact_item(shadow_field_all, 'block', 'BlockItem @Shadow field kind filter')
    assert_no_item(shadow_field_all, 'getBlock', 'BlockItem @Shadow field kind filter')

    local shadow_method_result = complete_lines(
      shadow_after_field, {shadow_method_row, shadow_method_column}, 'BlockItem @Shadow method completion')
    local shadow_method_item = exact_item(shadow_method_result, 'getBlock', 'BlockItem @Shadow method completion')
    h.helpers.assert_eq(shadow_method_item.insertText, 'getBlock')
    h.helpers.assert_eq(shadow_method_item.data.source, 'mixin.shadow')
    h.helpers.assert_not_nil(shadow_method_item.textEdit, 'BlockItem @Shadow method must provide a text edit')
    vim.lsp.util.apply_text_edits({shadow_method_item.textEdit}, bufnr, 'utf-16')
    local shadow_after_method = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    h.helpers.assert_eq(shadow_after_method[shadow_field_row], '    @Shadow private Block block;')
    h.helpers.assert_eq(shadow_after_method[shadow_method_row], '    @Shadow public abstract Block getBlock();')

    local shadow_method_all = complete_lines(
      shadow_after_method, {shadow_method_row, shadow_method_column - #'getBl'},
      'BlockItem @Shadow method kind filter')
    exact_item(shadow_method_all, 'getBlock', 'BlockItem @Shadow method kind filter')
    assert_no_item(shadow_method_all, 'block', 'BlockItem @Shadow method kind filter')
    assert_no_item(shadow_method_all, '<init>', 'BlockItem @Shadow method kind filter')
    assert_no_item(shadow_method_all, '<clinit>', 'BlockItem @Shadow method kind filter')

    -- The next provider-cache request belongs to the BlockItem injection
    -- fixture; do not let the Shadow declaration's extra rows leak into it.
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, block_lines)

    -- Exercise Blink's actual provider cache, not a mock of its incomplete flags.
    local blink_path = vim.env.MCDEV_E2E_BLINK_RTP
    h.helpers.assert_not_nil(blink_path, 'MCDEV_E2E_BLINK_RTP must point to the installed blink.cmp checkout')
    vim.opt.runtimepath:append(blink_path)
    local provider = require('blink.cmp.sources.lib.provider').new('mcdev', {module='mcdev.blink',timeout_ms=0})
    local function blink_request(prefix)
      local line = '    @Inject(method = "' .. prefix .. '")'
      local inject_row = block_inject_row
      vim.api.nvim_buf_set_lines(bufnr, inject_row - 1, inject_row, false, {line})
      local received, cached
      local start = #'    @Inject(method = "'
      local context = {id=1,bufnr=bufnr,mode='default',line=line,cursor={inject_row,start+#prefix},
        bounds={start_col=start+1,length=#prefix},trigger={kind=1,initial_kind='manual'}}
      provider:get_completions(context,function(value,is_cached) received,cached=value,is_cached end)
      h.helpers.assert_true(vim.wait(30000,function() return received ~= nil end,50),'Blink provider did not reply')
      return received,cached
    end
    local narrowed = blink_request('is')
    h.helpers.assert_eq(#narrowed,0, 'BlockItem has no declared is* injection target')
    local broadened,cached = blink_request('')
    h.helpers.assert_true(not cached, 'Blink reused the empty narrow-prefix response after backspace')
    h.helpers.assert_true(vim.iter(broadened):any(function(item) return item.data.name=='getPlacementState' end),
      'backspacing must restore actual BlockItem members through Blink')
    for _,item in ipairs(broadened) do h.helpers.assert_eq(item.source_id,'mcdev') end
    provider.list:destroy()

    -- Exercise Blink's real source tree and completion list across a context
    -- boundary.  A provider-only test cannot detect an old lsp/buffer list
    -- surviving after the route changes to a Mixin selector.
    local blink_cmp = require('blink.cmp')
    local blink_sources = require('blink.cmp.sources.lib')
    blink_cmp.setup({
      enabled = function() return true end,
      sources = {
        -- Match the installed user configuration: defer route_sources until
        -- Blink has installed its own CursorMovedI/TextChangedI listeners.
        default = function()
          return require('mcdev.blink').route_sources({'lsp', 'buffer', 'mcdev'})()
        end,
        providers = {
          -- Keep both real JDT and buffer providers in the normal aggregate;
          -- Blink's stock lsp->buffer fallback would otherwise never request
          -- the buffer source when JDT returns a method.
          lsp = {fallbacks = {}},
          mcdev = {module = 'mcdev.blink', timeout_ms = 0, score_offset = 100},
        },
      },
    })
    local blink_ready = vim.wait(30000, function()
      local trigger = package.loaded['blink.cmp.completion.trigger']
      return trigger ~= nil and trigger.buffer_events ~= nil
    end, 25)
    h.helpers.assert_true(blink_ready, 'Blink completion did not finish setup')

    local aggregate_lines = {
      'package io.github.cotrin8672.cem.mixin;',
      '',
      'import net.minecraft.world.item.Item;',
      'import org.spongepowered.asm.mixin.Mixin;',
      'import com.llamalad7.mixinextras.expression.Expression;',
      'import org.spongepowered.asm.mixin.injection.At;',
      'import org.spongepowered.asm.mixin.injection.Inject;',
      '',
      '@Mixin(Item.class)',
      'public class ItemMixin {',
      '    private static final String mixinBlinkBufferOnlySentinel = "world setReturnValue"; static final String getDefaultInstanceBufferOnly = "buffer";',
      '    @Inject(method = "")',
      '    private void mcdev$handler() {}',
      '    @Expression("")',
      '    void ordinary(Item item) {',
      '        item.getDef',
      '    }',
      '}',
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, aggregate_lines)

    -- Resolve every aggregate position from the exact fixture line once.  The
    -- imported At type is part of this fixture, so numeric rows are too easy
    -- to invalidate while adding another real annotation context.
    local ordinary_line = '        item.getDef'
    local selector_line = '    @Inject(method = "")'
    local mixin_line = '@Mixin(Item.class)'
    local expression_line = '    @Expression("")'
    local ordinary_row = row_of_exact(aggregate_lines, ordinary_line, 'ordinary Java expression')
    local selector_row = row_of_exact(aggregate_lines, selector_line, 'Mixin selector')
    local mixin_row = row_of_exact(aggregate_lines, mixin_line, 'Mixin class literal')
    local expression_row = row_of_exact(aggregate_lines, expression_line, 'MixinExtras expression value')

    local function provider_is_final(provider_id, context_id)
      local source_provider = blink_sources.get_provider_by_id(provider_id)
      local source_list = source_provider and source_provider.list
      return source_list ~= nil
        and source_list.context ~= nil
        and source_list.context.id == context_id
        and source_list.has_completed
    end

    local function item_has(items, predicate)
      for _, item in ipairs(items or {}) do
        if predicate(item) then return true end
      end
      return false
    end

    local function has_completion_text(item, expected)
      return item.insertText == expected
        or (item.textEdit ~= nil and item.textEdit.newText == expected)
    end

    local function selected_item_summary(items, expected)
      for _, item in ipairs(items or {}) do
        local data = item.data or {}
        if data.name == expected or item.label == expected or has_completion_text(item, expected) then
          return {
            label = item.label,
            insertText = item.insertText,
            textEditNewText = item.textEdit and item.textEdit.newText or nil,
            source_id = item.source_id,
            dataName = data.name,
            dataSource = data.source,
          }
        end
      end
      return nil
    end

    local function assert_exclusive_items(items, description)
      for _, item in ipairs(items) do
        h.helpers.assert_eq(item.source_id, 'mcdev', description .. ' must be owned exclusively by mcdev')
        h.helpers.assert_nil(item.label == 'world' and true or nil, 'buffer word leaked into ' .. description)
        h.helpers.assert_nil(item.label == 'setReturnValue' and true or nil,
          'buffer word leaked into ' .. description)
        h.helpers.assert_nil(item.label == 'mixinBlinkBufferOnlySentinel' and true or nil,
          'buffer sentinel leaked into ' .. description)
      end
    end

    local function wait_for_aggregate(row, column, expected_providers, expected_items, description)
      local route_ready = vim.wait(5000, function()
        local context = blink_cmp.get_context()
        if context == nil or context.cursor[1] ~= row or context.cursor[2] ~= column then
          return false
        end
        if #context.providers ~= #expected_providers then return false end
        for index, provider_id in ipairs(expected_providers) do
          if context.providers[index] ~= provider_id then return false end
        end
        return true
      end, 25)
      local context = blink_cmp.get_context()
      h.helpers.assert_true(route_ready,
        description .. ' route did not settle at ' .. row .. ':' .. column
          .. ': providers=' .. vim.inspect(context and context.providers or nil))

      local complete = vim.wait(30000, function()
        local current = blink_cmp.get_context()
        if current == nil or current.cursor[1] ~= row or current.cursor[2] ~= column then return false end
        for _, provider_id in ipairs(expected_providers) do
          if not provider_is_final(provider_id, current.id) then return false end
        end
        return expected_items(blink_cmp.get_items())
      end, 50)
      local items = vim.deepcopy(blink_cmp.get_items())
      local final_context = blink_cmp.get_context()
      if not complete then
        local provider_states = {}
        for _, provider_id in ipairs(expected_providers) do
          local provider = blink_sources.get_provider_by_id(provider_id)
          local source_list = provider and provider.list
          provider_states[provider_id] = {
            context_id = source_list and source_list.context and source_list.context.id or nil,
            has_completed = source_list and source_list.has_completed or nil,
            is_incomplete_forward = source_list and source_list.is_incomplete_forward or nil,
            item_count = source_list and #(source_list.items or {}) or nil,
          }
        end
        h.log_step(description .. ' aggregate diagnostic context=' .. vim.inspect({
          context_id = final_context and final_context.id or nil,
          cursor = final_context and final_context.cursor or nil,
          providers = final_context and final_context.providers or nil,
          providers_state = provider_states,
          HEAD = selected_item_summary(items, 'HEAD'),
          this = selected_item_summary(items, 'this'),
        }):gsub('%s+', ' '))
      end
      h.helpers.assert_true(complete,
        description .. ' aggregate completion did not finalize at ' .. row .. ':' .. column
          .. ': providers=' .. vim.inspect(final_context and final_context.providers or nil)
          .. ', items=' .. vim.inspect(vim.tbl_map(function(item) return item.label end, items)))
      return items
    end

    local function transition_aggregate(row, column, expected_providers, expected_items, description, event)
      h.helpers.assert_true(blink_cmp.is_active(), description .. ' must start with an active Blink menu')
      vim.api.nvim_win_set_cursor(0, {row, column})
      -- Dispatch the public editor event that the routing autocmd consumes.
      -- A query-outside cursor move is allowed to close Blink's menu.  If that
      -- happens, assert that no stale menu remains and use the public show API
      -- to model the user's next manual completion request.  If Blink stays
      -- active, the route watcher must update the existing list itself.
      local transition_event = event or 'CursorMovedI'
      vim.api.nvim_exec_autocmds(transition_event, {buffer = bufnr, modeline = false})
      if transition_event == 'CursorMovedI' and not blink_cmp.is_active() then
        h.helpers.assert_true(not blink_cmp.is_visible(),
          description .. ' must not leave a stale visible Blink menu after query-outside movement')
        blink_cmp.show()
      end
      return wait_for_aggregate(row, column, expected_providers, expected_items, description)
    end

    local function text_change_same_position(row, column, new_line, expected_providers, expected_items, description)
      h.helpers.assert_true(blink_cmp.is_active(), description .. ' must start with an active Blink menu')
      local before = vim.api.nvim_win_get_cursor(0)
      h.helpers.assert_eq(before[1], row, description .. ' must keep its row before TextChangedI')
      h.helpers.assert_eq(before[2], column, description .. ' must keep its column before TextChangedI')
      vim.api.nvim_buf_set_lines(bufnr, row - 1, row, false, {new_line})
      local after = vim.api.nvim_win_get_cursor(0)
      h.helpers.assert_eq(after[1], row, description .. ' must keep its row after TextChangedI')
      h.helpers.assert_eq(after[2], column, description .. ' must keep its column after TextChangedI')
      vim.api.nvim_exec_autocmds('TextChangedI', {buffer = bufnr, modeline = false})
      return wait_for_aggregate(row, column, expected_providers, expected_items, description)
    end

    local ordinary_column = #'        item.getDef'
    local aggregate_virtualedit = vim.o.virtualedit
    vim.o.virtualedit = 'onemore'
    local ordinary_items
    vim.api.nvim_win_set_cursor(0, {ordinary_row, ordinary_column})
    blink_cmp.show()
    ordinary_items = wait_for_aggregate(ordinary_row, ordinary_column, {'lsp', 'buffer'}, function(items)
      return item_has(items, function(item)
        return item.source_id == 'lsp'
          and (item.label or ''):find('getDefaultInstance', 1, true) ~= nil
      end)
    end, 'ordinary Java completion')
    h.helpers.assert_true(item_has(ordinary_items, function(item)
      return item.source_id == 'lsp'
        and (item.label or ''):find('getDefaultInstance', 1, true) ~= nil
    end), 'ordinary Java completion must retain the JDT method candidate')
    h.helpers.assert_true(item_has(ordinary_items, function(item)
      return item.source_id == 'buffer' and item.label == 'getDefaultInstanceBufferOnly'
    end), 'ordinary aggregate must visibly include the deterministic buffer candidate')
    local buffer_list = blink_sources.get_provider_by_id('buffer').list
    h.helpers.assert_true(buffer_list ~= nil and item_has(buffer_list.items, function(item)
      return item.insertText == 'mixinBlinkBufferOnlySentinel'
    end), 'ordinary aggregate must actually cache the irrelevant buffer sentinel')

    local selector_column = #'    @Inject(method = "'
    local mixin_items = transition_aggregate(selector_row, selector_column, {'mcdev'}, function(items)
      return item_has(items, function(item)
        return item.data and item.data.name == 'getEnchantmentValue'
      end)
    end, 'Mixin method selector')
    assert_exclusive_items(mixin_items, 'Mixin method selector')

    -- @Mixin's class literal deliberately keeps JDT available while mcdev
    -- contributes target-aware entries.  The buffer source must remain out.
    -- Put this transition before @At so every synthetic text change crosses a
    -- provider boundary and therefore exercises the public route refresh.
    vim.api.nvim_buf_set_lines(bufnr, mixin_row - 1, mixin_row, false, {'@Mixin(BlockI.class)'})
    local class_literal_items = transition_aggregate(mixin_row, #'@Mixin(BlockI', {'lsp', 'mcdev'}, function(items)
      return item_has(items, function(item)
        return item.source_id == 'lsp' and (item.label or ''):find('BlockItem', 1, true) ~= nil
      end)
    end, 'Mixin class literal', 'TextChangedI')
    for _, item in ipairs(class_literal_items) do
      h.helpers.assert_true(item.source_id == 'lsp' or item.source_id == 'mcdev',
        'unexpected source in Mixin class literal: ' .. tostring(item.source_id))
      h.helpers.assert_nil(item.label == 'mixinBlinkBufferOnlySentinel' and true or nil,
        'buffer sentinel leaked into Mixin class literal')
    end

    -- Keep the cursor fixed while changing the active context.  This is the
    -- boundary that catches a route watcher which only refreshes on movement.
    -- Keep the class-literal cursor (column 13) while placing the new text
    -- inside the unfinished `metho` attribute prefix of @Inject.  The
    -- shorter, unindented fixture makes that same byte position an actual
    -- attribute context without an existing key/value to suppress.
    local attribute_line = '@Inject(metho)'
    local same_position_items = text_change_same_position(
      mixin_row, #'@Mixin(BlockI', attribute_line, {'mcdev'}, function(items)
        return item_has(items, function(item)
          return item.data and item.data.source == 'mixin.attribute'
        end)
      end, 'same-position Mixin attribute boundary')
    assert_exclusive_items(same_position_items, 'same-position Mixin attribute boundary')

    -- Close the partial class-literal fixture before entering the nested
    -- contexts; the actual imported target is restored immediately after.
    local restored_class_items = text_change_same_position(
      mixin_row, #'@Mixin(BlockI', '@Mixin(BlockI.class)', {'lsp', 'mcdev'}, function(items)
        return item_has(items, function(item)
          return item.source_id == 'lsp' and (item.label or ''):find('BlockItem', 1, true) ~= nil
        end)
      end, 'restored Mixin class literal boundary')
    for _, item in ipairs(restored_class_items) do
      h.helpers.assert_true(item.source_id == 'lsp' or item.source_id == 'mcdev',
        'unexpected source in restored Mixin class literal: ' .. tostring(item.source_id))
    end

    -- Use a real imported target for the nested @At and Expression contexts;
    -- the preceding BlockI fixture intentionally tests a partial class name.
    vim.api.nvim_buf_set_lines(bufnr, mixin_row - 1, mixin_row, false, {'@Mixin(Item.class)'})
    local at_line = '    @Inject(method = "", at = @At(value = ""))'
    vim.api.nvim_buf_set_lines(bufnr, selector_row - 1, selector_row, false, {at_line})
    local at_column = #'    @Inject(method = "", at = @At(value = "'
    local at_items = transition_aggregate(selector_row, at_column, {'mcdev'}, function(items)
      return item_has(items, function(item)
        return item.data and item.data.source == 'mixin.atValue'
          and item.data.name == 'HEAD'
          and has_completion_text(item, 'HEAD')
      end)
    end, 'Mixin @At value', 'TextChangedI')
    assert_exclusive_items(at_items, 'Mixin @At value')

    local expression_items = transition_aggregate(expression_row, #'    @Expression("', {'mcdev'}, function(items)
      return item_has(items, function(item)
        return item.data and item.data.source == 'mixinextras.expressionValue'
          and item.data.name == 'this'
          and has_completion_text(item, 'this')
      end)
    end, 'MixinExtras Expression value')
    assert_exclusive_items(expression_items, 'MixinExtras Expression value')

    local ordinary_again = transition_aggregate(ordinary_row, ordinary_column, {'lsp', 'buffer'}, function(items)
      return item_has(items, function(item)
        return item.source_id == 'lsp'
          and (item.label or ''):find('getDefaultInstance', 1, true) ~= nil
      end)
    end, 'Java completion after Mixin context')
    h.helpers.assert_true(item_has(ordinary_again, function(item)
      return item.source_id == 'lsp'
        and (item.label or ''):find('getDefaultInstance', 1, true) ~= nil
    end), 'Java completion must recover after leaving the Mixin selector')
    blink_cmp.hide()
    local hidden = vim.wait(2000, function() return not blink_cmp.is_active() end, 25)
    h.helpers.assert_true(hidden, 'Blink aggregate menu did not close before the snippet check')
    vim.o.virtualedit = aggregate_virtualedit

    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, block_lines)
    vim.api.nvim_buf_set_lines(bufnr, block_inject_row - 1, block_inject_row, false, {'    @In'})
    local snippet_result
    local cancel_snippet = completion.complete(function(value)
      if not value.isProvisional then snippet_result=value end
    end,bufnr,{block_inject_row,#'    @In'},{source='real-cem-snippet',stream=true})
    local snippet_done = vim.wait(30000,function() return snippet_result ~= nil end,50)
    if not snippet_done then cancel_snippet() end
    h.helpers.assert_true(snippet_done,'CEM annotation template did not finalize')
    local snippet = vim.iter(snippet_result.items):find(function(item) return item.label=='Inject' end)
    h.helpers.assert_not_nil(snippet,'@In must offer the Inject template')
    h.helpers.assert_eq(snippet.insertTextFormat,vim.lsp.protocol.InsertTextFormat.Snippet)
    local edit = vim.deepcopy(snippet.textEdit)
    h.helpers.assert_not_nil(edit,'annotation template must supply its exact replacement range')
    edit.newText = ''
    vim.lsp.util.apply_text_edits({edit},bufnr,'utf-16')
    local virtualedit = vim.o.virtualedit
    vim.o.virtualedit = 'onemore'
    vim.api.nvim_win_set_cursor(0,{edit.range.start.line+1,edit.range.start.character})
    vim.snippet.expand(snippet.textEdit.newText)
    vim.o.virtualedit = virtualedit
    vim.lsp.util.apply_text_edits(snippet.additionalTextEdits or {},bufnr,'utf-16')
    local expanded_text = table.concat(vim.api.nvim_buf_get_lines(bufnr,0,-1,false),'\n')
    h.helpers.assert_true(expanded_text:find('@Inject(method = "", at = @At(""))',1,true) ~= nil,
      'native snippet expansion must create the complete Inject annotation: ' .. expanded_text)
    h.helpers.assert_true(expanded_text:find('import org.spongepowered.asm.mixin.injection.At;',1,true) ~= nil)
    vim.snippet.stop()

    local function expand_feature_snippet(prefix, label, expected_text, required_imports, description)
      local feature_lines = {
        'package io.github.cotrin8672.cem.mixin;',
        'import java.util.List;',
        'import net.minecraft.world.item.Item;',
        'import org.spongepowered.asm.mixin.Mixin;',
        '@Mixin(Item.class)',
        'public class ItemMixin {',
        '    @' .. prefix,
        '}',
      }
      local marker = '    @' .. prefix
      local row = row_of_exact(feature_lines, marker, description)
      local column = position_after(marker, marker, description)
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, feature_lines)
      vim.snippet.stop()

      local result
      local cancel = completion.complete(function(value)
        if not value.isProvisional then result = value end
      end, bufnr, {row, column}, {source='real-cem-feature-' .. prefix,stream=true})
      local finished = vim.wait(30000, function() return result ~= nil end, 50)
      if not finished then cancel() end
      h.helpers.assert_true(finished, description .. ' completion did not finalize')
      h.helpers.assert_nil(completion.last_error, description .. ' returned an error')
      h.helpers.assert_nil(completion.last_project_transport_error, description .. ' project transport failed')

      local snippet = vim.iter(result.items or {}):find(function(item)
        return item.label == label
      end)
      h.helpers.assert_not_nil(snippet, description .. ' missing ' .. label .. ' snippet')
      h.helpers.assert_eq(snippet.insertTextFormat, vim.lsp.protocol.InsertTextFormat.Snippet,
        description .. ' must be a native snippet')
      h.helpers.assert_eq(snippet.data and snippet.data.source, 'mixin.annotation',
        description .. ' must come from the annotation snippet provider')
      local edit = vim.deepcopy(snippet.textEdit)
      h.helpers.assert_not_nil(edit, description .. ' must supply a replacement range')
      h.helpers.assert_not_nil(edit.newText, description .. ' must supply snippet text')
      edit.newText = ''
      vim.lsp.util.apply_text_edits({edit}, bufnr, 'utf-16')
      local virtualedit = vim.o.virtualedit
      vim.o.virtualedit = 'onemore'
      vim.api.nvim_win_set_cursor(0, {edit.range.start.line + 1, edit.range.start.character})
      local ok, err = pcall(vim.snippet.expand, snippet.textEdit.newText)
      vim.o.virtualedit = virtualedit
      h.helpers.assert_true(ok, description .. ' native snippet expansion failed: ' .. tostring(err))
      h.helpers.assert_true(type(vim.snippet.active) == 'function' and vim.snippet.active({direction = 1}),
        description .. ' must enter a native snippet tabstop')
      vim.lsp.util.apply_text_edits(snippet.additionalTextEdits or {}, bufnr, 'utf-16')

      local expanded = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), '\n')
      h.helpers.assert_true(expanded:find(expected_text, 1, true) ~= nil,
        description .. ' default expansion mismatch: ' .. expanded)
      for _, import_name in ipairs(required_imports) do
        h.helpers.assert_true(expanded:find(import_name, 1, true) ~= nil,
          description .. ' missing required import ' .. import_name)
      end
      for _, preserved in ipairs({
        'package io.github.cotrin8672.cem.mixin;',
        'import java.util.List;',
        'import net.minecraft.world.item.Item;',
        '@Mixin(Item.class)',
        'public class ItemMixin {',
      }) do
        h.helpers.assert_true(expanded:find(preserved, 1, true) ~= nil,
          description .. ' did not preserve ' .. preserved)
      end
      vim.snippet.stop()
      h.helpers.assert_eq(table.concat(vim.fn.readfile(h.mixin_file), '\n'), disk_source,
        description .. ' must not modify the project source on disk')
      return expanded
    end

    expand_feature_snippet(
      'Im', 'Implements',
      'Implements({ @Interface(iface = Target.class, prefix = "prefix$") })',
      {'import org.spongepowered.asm.mixin.Implements;', 'import org.spongepowered.asm.mixin.Interface;'},
      'nested Implements snippet')
    expand_feature_snippet(
      'Descr', 'Descriptors',
      'Descriptors({ @Desc(value = "", owner = Target.class, ret = void.class, args = {  }) })',
      {'import org.spongepowered.asm.mixin.injection.Descriptors;', 'import org.spongepowered.asm.mixin.injection.Desc;'},
      'grouped Descriptors snippet')
    expand_feature_snippet(
      'Grou', 'Group',
      'Group(name = "", min = -1, max = -1)',
      {'import org.spongepowered.asm.mixin.injection.Group;'},
      'Group whole-name replacement snippet')
    h.log_step('passed native CEM Implements, Descriptors, and Group snippets')

    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, original)
    vim.bo[bufnr].modified = false
    h.helpers.assert_eq(table.concat(vim.fn.readfile(h.mixin_file),'\n'),disk_source,'E2E must not modify the project source on disk')
    h.log_step('passed actual CEM target members, selector edits, and native feature snippets')
  end)
end
