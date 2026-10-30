-- Shared original CEM snippet group; member/Shadow/Blink scenarios stay in cem_project_e2e.lua.
local M = {}
M.inject_lines = {
  'package io.github.cotrin8672.cem.mixin;',
  'import net.minecraft.world.item.BlockItem;',
  'import org.spongepowered.asm.mixin.Mixin;',
  'import org.spongepowered.asm.mixin.injection.Inject;',
  '@Mixin(BlockItem.class)',
  'public class ItemMixin {',
  '    @Inject(method = "")',
  '}',
}
local templates = {
  Inject = 'Inject(method = "${1}", at = @At("${2}"))$0',
  Implements = 'Implements({ @Interface(iface = ${1:Target}.class, prefix = "${2:prefix\\$}") })$0',
  Descriptors = 'Descriptors({ @Desc(value = "${1}", owner = ${2:Target}.class, ret = ${3:void}.class, args = { ${4} }) })$0',
  Group = 'Group(name = "${1}", min = ${2:-1}, max = ${3:-1})$0',
}
function M.run(h, bufnr, block_lines, block_inject_row, disk_source, observations)
  local completion = require('mcdev.completion')
  local function row_of_exact(lines, exact, description)
    for row, line in ipairs(lines) do
      if line == exact then return row end
    end
    h.helpers.assert_true(false, 'missing fixture line for ' .. description .. ': ' .. exact)
  end
  local function position_after(line, marker, description)
    local start = line:find(marker, 1, true)
    h.helpers.assert_not_nil(start, 'missing caret marker for ' .. description)
    return start - 1 + #marker
  end
  local function exact_expansion(label, input, row, prefix, snippet, expanded, default, imports)
    h.helpers.assert_eq(snippet.label, label)
    h.helpers.assert_eq(snippet.filterText, label:lower() .. ' ' .. label, label .. ' exact filter text')
    h.helpers.assert_eq(snippet.insertText, templates[label], label .. ' exact snippet and tabstops')
    h.helpers.assert_true(vim.deep_equal(snippet.textEdit, {
      range = { start = { line = row - 1, character = 5 },
        ['end'] = { line = row - 1, character = 5 + #prefix } },
      newText = templates[label],
    }), label .. ' exact annotation-name replacement')
    local import_row
    for index, line in ipairs(input) do
      if line:match('^import ') then import_row = index end
    end
    local point = { line = import_row - 1, character = #input[import_row] }
    h.helpers.assert_true(vim.deep_equal(snippet.additionalTextEdits, {{
      range = { start = point, ['end'] = vim.deepcopy(point) },
      newText = '\n' .. table.concat(imports, '\n') .. '\n',
    }}), label .. ' exact additional import edit')
    local expected = vim.deepcopy(input)
    expected[row] = '    @' .. default
    for index, name in ipairs(imports) do table.insert(expected, import_row + index, name) end
    table.insert(expected, import_row + #imports + 1, '')
    h.helpers.assert_eq(expanded, table.concat(expected, '\n'), label .. ' complete expanded buffer and indentation')
    h.helpers.assert_true(vim.snippet.active({direction = 1}), label .. ' must keep a native tabstop active')
    h.helpers.assert_true(vim.bo[bufnr].modified, label .. ' expansion must stay unsaved')
    h.helpers.assert_eq(table.concat(vim.fn.readfile(h.mixin_file), '\n'), disk_source,
      label .. ' must leave disk content unchanged')
    observations[#observations + 1] = {
      label = label, input = vim.deepcopy(input), item = vim.deepcopy(snippet), expanded = expanded,
      nativeTabstop = true, unsaved = true, diskUnchanged = true,
    }
  end
  h.helpers.assert_true(vim.deep_equal(block_lines, M.inject_lines), 'Inject must use the original BlockItem input')
  local inject_lines = vim.deepcopy(block_lines)
  inject_lines[block_inject_row] = '    @In'
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
  local returned_snippet = vim.deepcopy(snippet)
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
  exact_expansion('Inject', inject_lines, block_inject_row, 'In', returned_snippet, expanded_text,
    'Inject(method = "", at = @At(""))', {'import org.spongepowered.asm.mixin.injection.At;'})
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
    local returned_snippet = vim.deepcopy(snippet)
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
    exact_expansion(label, feature_lines, row, prefix, returned_snippet, expanded, expected_text, required_imports)
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
  return observations
end
return M
