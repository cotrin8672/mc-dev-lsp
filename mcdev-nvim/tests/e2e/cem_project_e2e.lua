return function(h)
  local completion = require('mcdev.completion')
  local disk_source = table.concat(vim.fn.readfile(h.mixin_file),'\n')
  h.with_buffer(h.mixin_file, 'java', nil, function(bufnr)
    local original = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    -- Use the real project's Minecraft/NeoForge classes. These unfinished editor
    -- buffers are never written to the user's project.
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
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
      local result
      local cancel = completion.complete(function(value)
        if not value.isProvisional then result = value end
      end, bufnr, {7, #'    @Inject(method = "' + #prefix}, {source='real-cem',stream=true})
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
      h.log_step('CEM ' .. owner .. ':' .. prefix .. ' final candidates=' .. #(result.items or {}))
      return by_name, lines
    end
    local items = complete('Item', 'is')
    h.helpers.assert_not_nil(items.isEnchantable)
    h.helpers.assert_nil(items.getEnchantmentValue)
    -- Deleting the prefix must expand the final result, including methods that
    -- never appeared in the narrow response.
    local expanded, lines = complete('Item', '')
    for _,name in ipairs({'isEnchantable','getEnchantmentValue','getDefaultInstance','use','<init>'}) do
      h.helpers.assert_not_nil(expanded[name], 'missing Item.' .. name)
    end
    local chosen = expanded.getEnchantmentValue
    h.helpers.assert_not_nil(chosen.textEdit)
    vim.lsp.util.apply_text_edits({chosen.textEdit}, bufnr, 'utf-16')
    lines[7] = '    @Inject(method = "getEnchantmentValue")'
    h.helpers.assert_eq(table.concat(vim.api.nvim_buf_get_lines(bufnr,0,-1,false),'\n'), table.concat(lines,'\n'))
    local block = complete('BlockItem', '')
    for _,name in ipairs({'getBlock','getPlacementState','canPlace','<init>'}) do
      h.helpers.assert_not_nil(block[name], 'missing BlockItem.' .. name)
    end
    h.helpers.assert_nil(block.isEnchantable, 'inherited methods are not injection targets declared in BlockItem')

    -- Exercise Blink's actual provider cache, not a mock of its incomplete flags.
    local blink_path = vim.env.MCDEV_E2E_BLINK_RTP
    h.helpers.assert_not_nil(blink_path, 'MCDEV_E2E_BLINK_RTP must point to the installed blink.cmp checkout')
    vim.opt.runtimepath:append(blink_path)
    local provider = require('blink.cmp.sources.lib.provider').new('mcdev', {module='mcdev.blink',timeout_ms=0})
    local function blink_request(prefix)
      local line = '    @Inject(method = "' .. prefix .. '")'
      vim.api.nvim_buf_set_lines(bufnr,6,7,false,{line})
      local received, cached
      local start = #'    @Inject(method = "'
      local context = {id=1,bufnr=bufnr,mode='default',line=line,cursor={7,start+#prefix},
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

    vim.api.nvim_buf_set_lines(bufnr,6,7,false,{'    @In'})
    local snippet_result
    local cancel_snippet = completion.complete(function(value)
      if not value.isProvisional then snippet_result=value end
    end,bufnr,{7,#'    @In'},{source='real-cem-snippet',stream=true})
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
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, original)
    vim.bo[bufnr].modified = false
    h.helpers.assert_eq(table.concat(vim.fn.readfile(h.mixin_file),'\n'),disk_source,'E2E must not modify the project source on disk')
    h.log_step('passed actual CEM target members, prefix deletion and applied selector edit')
  end)
end
