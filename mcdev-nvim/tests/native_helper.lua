local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local stdio = require("mcdev.stdio")
local jobstart, chansend = vim.fn.jobstart, vim.fn.chansend
local prefix = "build/nvim-test-logs/native-helper-child"
vim.fn.writefile({}, prefix .. ".stdout.log")
vim.fn.writefile({}, prefix .. ".requests.jsonl")
vim.fn.jobstart = function(command, options)
  local on_stdout = options.on_stdout
  options.on_stdout = function(id, data, event)
    vim.fn.writefile({ table.concat(data or {}, "\n") }, prefix .. ".stdout.log", "ab")
    on_stdout(id, data, event)
  end
  return jobstart(command, options)
end
vim.fn.chansend = function(id, data)
  vim.fn.writefile({ data }, prefix .. ".requests.jsonl", "ab")
  return chansend(id, data)
end
vim.api.nvim_create_autocmd("VimLeavePre", { callback = function() stdio.stop() end })
local binary = vim.env.MCDEV_COMPANION_BINARY
  or (vim.fn.getcwd() .. "/target/debug/mcdev-companion.exe")
h.assert_eq(vim.fn.executable(binary), 1, "build the native companion before this process test")
local opts = { companion = { cmd = { binary } }, timeout_ms = 5000 }
local function payload(source, character)
  return { context = {
    protocolVersion = 1, workspaceRoot = "file:///workspace", documentUri = "file:///workspace/Mixin.java",
    languageId = "java", position = { line = 0, character = character }, bufferTextFallback = source,
    documentVersion = 1, client = { name = "mcdev-nvim-native-test", version = "1" },
  }, trigger = { kind = "manual" }, options = {
    preferredAtTarget = "smart", mixinClassInsert = "import", injectMethodDescriptor = "auto",
  } }
end
local function request(source, character)
  local result, err, done
  local cancel, start_error = stdio.request(payload(source, character), function(value, failure)
    result, err, done = value, failure, true
  end, opts)
  h.assert_nil(start_error)
  h.assert_not_nil(cancel)
  h.assert_true(vim.wait(6000, function() return done end, 10), "native helper response must settle")
  return result, err
end
local first, first_error = request("@Inject(meth", 12)
h.assert_nil(first_error)
h.assert_eq(first.result.items[1].edit.range.start.character, 8)
h.assert_eq(first.result.items[1].edit.range["end"].character, 12)
local second, second_error = request('@Expression("{ ret', 18)
h.assert_nil(second_error)
h.assert_true(#second.result.items[1].edit.newText > 0)
local job = assert(stdio.start(opts))
vim.fn.chansend(job, "{malformed\n")
h.assert_true(vim.wait(2000, function() return stdio.last_error ~= nil end, 10))
h.assert_eq(stdio.last_error, "mcdev: invalid stdio helper response")
local recovered, recovered_error = request("@Inject(meth", 12)
h.assert_nil(recovered_error)
h.assert_true(#recovered.result.items > 0)
local source = '@Inject(method = "tick", at = @At(value = "INVOKE", target = ""))'
local unsupported, unsupported_error = request(source, source:find('target = "', 1, true) + #'target = "' - 1)
h.assert_nil(unsupported)
h.assert_true(type(unsupported_error) == "string")
vim.fn.chanclose(job, "stdin")
h.assert_eq(vim.fn.jobwait({ job }, 5000)[1], 0, "native helper must exit cleanly at EOF")
stdio.stop()
vim.fn.writefile({ stdio.last_stderr }, "build/nvim-test-logs/native-helper-child.stderr.log")
vim.fn.jobstart, vim.fn.chansend = jobstart, chansend
print("native helper Neovim transport: five requests, malformed recovery and EOF passed")
