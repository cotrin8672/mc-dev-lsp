local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local blink = require("mcdev.blink")
local buffer = require("mcdev.buffer")

local original_bufnr = vim.api.nvim_get_current_buf()
local defaults = { "snippets", "lazydev", "copilot", "buffer", "path", "lsp", "mcdev" }
local routed_sources = blink.route_sources(defaults)

local function assert_sources(actual, expected, message)
  helpers.assert_eq(#actual, #expected, message)
  for index, source_name in ipairs(expected) do
    helpers.assert_eq(actual[index], source_name, message)
  end
end

local function with_buffer(lines, callback)
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_name(bufnr, "/project/src/main/java/com/example/Example.java")
  vim.bo[bufnr].filetype = "java"
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.api.nvim_set_current_buf(bufnr)
  callback(bufnr)
  vim.api.nvim_set_current_buf(original_bufnr)
  vim.api.nvim_buf_delete(bufnr, { force = true })
end

with_buffer({ '@Inject(method = "foo"' }, function(bufnr)
  local position = { 1, #'@Inject(method = "foo"' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "mcdev" })
end)

with_buffer({ "@Inject(method = \"foo\")", "import com.example.World;" }, function(bufnr)
  local position = { 2, #"import com.example.World;" }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
  helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false)
  assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
end)

with_buffer({ 'String text = "@Inject";' }, function(bufnr)
  local position = { 1, #'String text = "@Inject";' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false)
  assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
end)

with_buffer({ "@Inject" }, function(bufnr)
  local position = { 1, #"@Inject" }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "lsp", "mcdev" })
end)

with_buffer({ "@Inject(", "method = \"foo\"" }, function(bufnr)
  local position = { 2, #'method = "foo"' }
  vim.api.nvim_win_set_cursor(0, position)
  helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position))
  assert_sources(routed_sources(), { "mcdev" })
end)

for _, annotation in ipairs({ "Wrap", "WrapOperation", "Expression", "Definition", "Local", "Share" }) do
  with_buffer({ "@" .. annotation .. "(value = \"foo\"" }, function(bufnr)
    local position = { 1, #("@" .. annotation .. "(value = \"foo\"") }
    helpers.assert_true(buffer.is_mcdev_completion_context(bufnr), annotation)
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), annotation)
  end)
end

-- Java values inside a Mixin annotation remain mixed with JDT.  These are
-- class literals, enum constants, and nested annotation names, not Mixin DSL
-- strings.
for _, case in ipairs({
  { "@Mixin(Item.cla", "class literal" },
  { "@Mixin({Item.class, Other.cla", "class literal array" },
  { "@Mixin(In", "Mixin class prefix collision" },
  { "@Mixin(Re", "Mixin class prefix collision" },
  { "@At(In", "At value prefix collision" },
  { "@Definition(type = String.cla", "Definition type" },
  { "@Inject(locals = LocalCapture.CAP", "enum constant" },
  { "@Inject(at = @At", "nested annotation name" },
  { "@Inject(method = TARGET_METHOD", "static final selector" },
  { "@Inject(method = {TARGET_METHOD", "static final selector array" },
  { '@Inject(method = "prefix" + TARGET_METHOD', "string constant expression" },
  { '@Inject(method = {"tick" + TARGET_METHOD', "string array constant expression" },
  { '@At("INVOKE" + AT_SUFFIX', "At shorthand string constant expression" },
  { '@Expression("this." + EXPRESSION_SUFFIX', "Expression shorthand string constant expression" },
  { "@At(target = TARGET_TARGET", "static final At target" },
  { "@ModifyConstant(constant = CONSTANT", "constant expression" },
  { "@Constant(stringValue = STRING_CONSTANT", "constant string expression" },
}) do
  with_buffer({ case[1] }, function(bufnr)
    local position = { 1, #case[1] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "lsp", "mcdev" })
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), case[2])
  end)
end

-- The core Mixin annotations that only have name snippets remain mixed once
-- their body starts, so JDT LS still owns Java attributes and values.
for _, case in ipairs({
  { "@Group(", "Group body" },
  { "@Debug(", "Debug body" },
  { "@Dynamic(", "Dynamic body" },
  { "@Unique(", "Unique body" },
  { "@Intrinsic(", "Intrinsic body" },
  { "@Implements({ @Interface(", "nested Interface body" },
  { "@Desc(", "Desc body" },
  { "@Descriptors({ @Desc(", "nested Desc body" },
}) do
  with_buffer({ case[1] }, function(bufnr)
    local position = { 1, #case[1] }
    vim.api.nvim_win_set_cursor(0, position)
    local context = buffer.completion_context_at(bufnr, position)
    helpers.assert_true(context ~= nil, case[2])
    helpers.assert_eq(context.exclusive, false, case[2])
    assert_sources(routed_sources(), { "lsp", "mcdev" }, case[2])
  end)
end

for _, case in ipairs({
  { "@Gro", "Group name prefix" },
  { "@Implem", "Implements name prefix" },
  { "@org.spongepowered.asm.mixin.injection.Gr", "Group FQN name prefix" },
  { "@Descriptors({ @Desc", "nested Desc name prefix" },
}) do
  with_buffer({ case[1] }, function(bufnr)
    local position = { 1, #case[1] }
    vim.api.nvim_win_set_cursor(0, position)
    local context = buffer.completion_context_at(bufnr, position)
    helpers.assert_true(context ~= nil, case[2])
    helpers.assert_eq(context.kind, "annotation_name", case[2])
    assert_sources(routed_sources(), { "lsp", "mcdev" }, case[2])
  end)
end

local normal_sources = { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" }
for _, case in ipairs({
  {
    { "@Overwrite public Block getBlock(){return null;}" },
    1,
    #'@Overwrite public Block getBlo',
    { "mcdev" },
    true,
    "complete Overwrite member header",
  },
  {
    { "@Overwrite public Block getBlo" },
    1,
    #'@Overwrite public Block getBlo',
    { "mcdev" },
    true,
    "incomplete Overwrite member header",
  },
  {
    { "@Shadow public Block getBlock(){return null;}" },
    1,
    #'@Shadow public Block getBlo',
    { "mcdev" },
    true,
    "complete Shadow method header",
  },
  {
    { "@Shadow public Block getBlo" },
    1,
    #'@Shadow public Block getBlo',
    { "lsp", "mcdev" },
    true,
    "incomplete Shadow member header",
  },
  {
    { "@Overwrite public Block getBlock(){return null;}" },
    1,
    #'@Overwrite public Blo',
    normal_sources,
    false,
    "return type stays JDT-owned",
  },
  {
    { "@Overwrite public Block getBlock(){return null;}" },
    1,
    #'@Overwrite public Block getBlock(){return nul',
    normal_sources,
    false,
    "method body stays ordinary Java",
  },
  {
    { "@Overwrite public Block getBlock(){return null;}", "public Block other" },
    2,
    #"public Block other",
    normal_sources,
    false,
    "next member stays ordinary Java",
  },
  {
    { "@Shadow private Block block;" },
    1,
    #'@Shadow private Block blo',
    { "mcdev" },
    true,
    "complete Shadow field header",
  },
  {
    { "@Shadow @Final private Block block;" },
    1,
    #'@Shadow @Final private Block blo',
    { "mcdev" },
    true,
    "Shadow field with a following annotation",
  },
  {
    { "@Shadow @Nullable public Block getBlock(){return null;}" },
    1,
    #'@Shadow @Nullable public Block getBlo',
    { "mcdev" },
    true,
    "Shadow method with a following annotation",
  },
  {
    { "@Shadow @Nullable public Blo" },
    1,
    #'@Shadow @Nullable public Blo',
    normal_sources,
    false,
    "Shadow return type with a following annotation stays Java",
  },
  {
    { "@Shadow private Block blo" },
    1,
    #'@Shadow private Block blo',
    { "lsp", "mcdev" },
    true,
    "incomplete Shadow field header",
  },
}) do
  with_buffer(case[1], function(bufnr)
    local position = { case[2], case[3] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), case[4], case[6])
    if case[5] then
      helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), case[6])
    else
      helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false, case[6])
    end
  end)
end

for _, case in ipairs({
  { { "@org.spongepowered.asm.mixin.injection.Inject(method = \"ti" }, "official FQN" },
  { { "import org.spongepowered.asm.mixin.injection.Inject;", "@Inject(method = \"ti" }, "official import" },
}) do
  with_buffer(case[1], function(bufnr)
    local position = { #case[1], #case[1][#case[1]] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "mcdev" })
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), case[2])
  end)
end

for _, case in ipairs({
  { { "@org.spongepowered.asm.mixin.injection.Group(" }, "new official FQN" },
  { { "import org.spongepowered.asm.mixin.injection.Group;", "@Group(" }, "new official import" },
}) do
  with_buffer(case[1], function(bufnr)
    local position = { #case[1], #case[1][#case[1]] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "lsp", "mcdev" }, case[2])
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), case[2])
  end)
end

for _, case in ipairs({
  { { "@com.example.Inject(method = \"ti" }, "non-Mixin FQN" },
  { { "import com.example.Inject;", "@Inject(method = \"ti" }, "non-Mixin import" },
  { { "@com.example.Group(" }, "new non-Mixin FQN" },
  { { "import com.example.Group;", "@Group(" }, "new non-Mixin import" },
}) do
  with_buffer(case[1], function(bufnr)
    local position = { #case[1], #case[1][#case[1]] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
    helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false, case[2])
  end)
end

-- Strings and annotation attributes owned by Mixin are exclusive.  The
-- parser must also ignore decoys in comments and ordinary Java strings.
for _, case in ipairs({
  { '@Inject(method = "ti', "method selector" },
  { '@Inject(method = {"tick", "re', "method array" },
  { '@Inject(method = {"tick", ', "method array next element" },
  { '@Inject(method = {"tick", /* comma, */ ', "array comment comma" },
  { '@Inject(at = @At(value = "INVOKE", target = "Lcom/example/', "At target" },
  { '@Expression("this.', "Expression DSL" },
  { '@Definition(id = "ma', "Definition id" },
  { '@Definition(method = "Lcom/example/Foo;', "Definition method" },
  { '@Inject(method = ', "incomplete method" },
  { '@Inject(/* comment */ method = "ti', "comment before method" },
}) do
  with_buffer({ case[1] }, function(bufnr)
    local position = { 1, #case[1] }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "mcdev" })
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), case[2])
  end)
end

for _, line in ipairs({
  '// @Inject(method = "fake',
  'String value = "@Inject(method = \\"fake";',
}) do
  with_buffer({ line }, function(bufnr)
    local position = { 1, #line }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "snippets", "lazydev", "copilot", "buffer", "path", "lsp" })
    helpers.assert_eq(buffer.is_mcdev_completion_context_at(bufnr, position), false)
  end)
end

with_buffer({ "@Cancellable" }, function(bufnr)
  helpers.assert_true(buffer.is_mcdev_completion_context(bufnr))
end)

local completion = require("mcdev.completion")
local original_complete = completion.complete
local source_requests = 0
completion.complete = function(callback)
  source_requests = source_requests + 1
  callback({ items = { { label = "annotation", insertText = "annotation" } } })
end

for _, prefix in ipairs({ "", "Mix", "In", "At", "Wrap", "Expr", "Definition", "Local", "Cancellable" }) do
  with_buffer({ "@" .. prefix .. " " }, function(bufnr)
    local position = { 1, #("@" .. prefix) }
    vim.api.nvim_win_set_cursor(0, position)
    assert_sources(routed_sources(), { "lsp", "mcdev" })
    local result
    blink.new():get_completions({ bufnr = bufnr, cursor = position }, function(value)
      result = value
    end)
    helpers.assert_true(buffer.is_mcdev_completion_context_at(bufnr, position), prefix)
    helpers.assert_eq(#result.items, 1, prefix)
  end)
end

helpers.assert_eq(source_requests, 9)
completion.complete = original_complete

print("mcdev-nvim blink routing tests passed")
