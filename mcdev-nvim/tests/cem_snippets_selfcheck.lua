-- Mock completion only checks the shared native-expansion test; it supplies no live-host evidence.
local helpers = dofile(vim.fn.getcwd() .. '/mcdev-nvim/tests/test_helpers.lua')
local group = dofile(vim.fn.getcwd() .. '/mcdev-nvim/tests/e2e/cem_feature_snippets.lua')
for _, file in ipairs({ 'cem_project_e2e.lua', 'cem_feature_snippets.lua', 'cem_snippets_host.lua' }) do
  assert(loadfile(vim.fn.getcwd() .. '/mcdev-nvim/tests/e2e/' .. file), file .. ' syntax')
end
local completion = require('mcdev.completion')
local original_complete, original_record = completion.complete, _G.mcdev_record_assertion
local original_error, original_transport_error = completion.last_error, completion.last_project_transport_error
local original_bufnr = vim.api.nvim_get_current_buf()
local original_window = vim.api.nvim_get_current_win()
local original_cursor = vim.api.nvim_win_get_cursor(original_window)
local original_virtualedit = vim.o.virtualedit
local root = vim.fn.getcwd() .. '/build/nvim-test-logs/cem-snippet-selfcheck-' .. vim.fn.getpid()
vim.fn.mkdir(root, 'p')
local path = root .. '/ItemMixin.java'
local disk_source = '// saved source must stay untouched'
vim.fn.writefile({ disk_source }, path)
local bufnr = vim.fn.bufadd(path)
vim.fn.bufload(bufnr)
local by_label = {}
local labels = { In = 'Inject', Im = 'Implements', Descr = 'Descriptors', Grou = 'Group' }
local macro_imports = {
  ['@Mixin:INJECT'] = 'org.spongepowered.asm.mixin.injection.Inject',
  ['@Mixin:AT'] = 'org.spongepowered.asm.mixin.injection.At',
}
local mutation, requests = nil, {}
local ok, failure = xpcall(function()
  vim.api.nvim_set_current_buf(bufnr)
  vim.bo[bufnr].filetype = 'java'
  local templates = vim.json.decode(table.concat(vim.fn.readfile(
    vim.fn.getcwd() .. '/mcdev-core/src/mixinextras_expression/feature_snippets.json'), '\n'))
  for _, template in ipairs(templates) do by_label[template.label] = template end
  completion.complete = function(callback, requested_bufnr, position)
    assert(requested_bufnr == bufnr)
    local lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    local prefix = assert(lines[position[1]]:match('^    @(%a+)$'))
    local template = assert(by_label[labels[prefix]])
    requests[#requests + 1] = { label = template.label, input = lines, position = position }
    local import_row, existing = nil, {}
    for row, line in ipairs(lines) do
      if line:match('^import ') then import_row, existing[line] = row, true end
    end
    local imports = {}
    for _, name in ipairs(template.imports) do
      local line = 'import ' .. (macro_imports[name] or name) .. ';'
      if not existing[line] then imports[#imports + 1] = line end
    end
    local point = { line = import_row - 1, character = #lines[import_row] }
    local item = {
      label = template.label, filterText = template.trigger .. ' ' .. template.label,
      insertText = template.insertText, insertTextFormat = vim.lsp.protocol.InsertTextFormat.Snippet,
      data = { source = template.source },
      textEdit = { range = { start = { line = position[1] - 1, character = 5 },
        ['end'] = { line = position[1] - 1, character = position[2] } }, newText = template.insertText },
      additionalTextEdits = {{ range = { start = point, ['end'] = vim.deepcopy(point) },
        newText = '\n' .. table.concat(imports, '\n') .. '\n' }},
    }
    completion.last_error, completion.last_project_transport_error = nil, nil
    if mutation == 'filter' then item.filterText = 'wrong filter' end
    if mutation == 'transport' then completion.last_project_transport_error = 'completion transport unavailable' end
    callback({ items = { item }, isProvisional = false })
    return function() end
  end
  _G.mcdev_record_assertion = function() end
  local records = group.run({ helpers = helpers, mixin_file = path, log_step = function() end },
    bufnr, group.inject_lines, 7, disk_source, {})
  _G.mcdev_record_assertion = original_record
  helpers.assert_eq(#records, 4)
  helpers.assert_eq(#requests, 4)
  for index, label in ipairs({ 'Inject', 'Implements', 'Descriptors', 'Group' }) do
    helpers.assert_eq(records[index].label, label)
    helpers.assert_true(records[index].nativeTabstop and records[index].unsaved and records[index].diskUnchanged)
  end
  for _, case in ipairs({ { 'filter', 'exact filter text' }, { 'transport', 'project transport failed' } }) do
    mutation = case[1]
    _G.mcdev_record_assertion = function() end
    local passed, err = pcall(group.run, { helpers = helpers, mixin_file = path, log_step = function() end },
      bufnr, group.inject_lines, 7, disk_source, {})
    _G.mcdev_record_assertion = original_record
    vim.snippet.stop()
    helpers.assert_true(not passed and tostring(err):find(case[2], 1, true) ~= nil,
      'shared original snippet assertions must reject ' .. case[1] .. ': ' .. tostring(err))
  end
end, debug.traceback)
vim.snippet.stop()
completion.complete, _G.mcdev_record_assertion = original_complete, original_record
completion.last_error, completion.last_project_transport_error = original_error, original_transport_error
vim.o.virtualedit = original_virtualedit
vim.api.nvim_set_current_win(original_window)
vim.api.nvim_set_current_buf(original_bufnr)
vim.api.nvim_win_set_cursor(original_window, original_cursor)
vim.api.nvim_buf_delete(bufnr, { force = true })
if not ok then error(failure) end
print('CEM snippet syntax and mock native expansion checks passed; no live-host assertions credited')
return 4
