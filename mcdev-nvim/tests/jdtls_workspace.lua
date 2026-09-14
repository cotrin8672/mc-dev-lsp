local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local jdtls = require("mcdev.jdtls")

local original_start = vim.lsp.start
local original_resolve_extension_jar = jdtls.resolve_extension_jar
local original_executable = vim.fn.executable
local original_filereadable = vim.fn.filereadable
local original_mkdir = vim.fn.mkdir
local fake_jar = "mcdev-jdtls-workspace-test.jar"
local started = {}
local mkdir_calls = {}

vim.lsp.start = function(start_opts)
  started[#started + 1] = start_opts
  return #started
end
jdtls.resolve_extension_jar = function()
  return fake_jar
end
vim.fn.executable = function(path)
  return path == "jdtls" and 1 or 0
end
vim.fn.filereadable = function(path)
  return path == fake_jar and 1 or original_filereadable(path)
end
vim.fn.mkdir = function(path, mode)
  mkdir_calls[#mkdir_calls + 1] = { path, mode }
end

local function expected_data_dir(root_dir)
  return vim.fn.stdpath("cache")
    .. "/mcdev-jdtls/"
    .. vim.fn.sha256(vim.fs.normalize(vim.fn.fnamemodify(root_dir, ":p")))
end

local root_a = "mcdev-jdtls-workspace-a"
local root_b = "mcdev-jdtls-workspace-b"
helpers.assert_eq(jdtls.start_or_attach({ root_dir = root_a }), 1)
helpers.assert_eq(started[1].cmd[1], "jdtls")
helpers.assert_eq(started[1].cmd[2], "-data")
helpers.assert_eq(started[1].cmd[3], expected_data_dir(root_a))

helpers.assert_eq(jdtls.start_or_attach({ root_dir = "./" .. root_a }), 2)
helpers.assert_eq(started[2].cmd[3], started[1].cmd[3])

helpers.assert_eq(jdtls.start_or_attach({ root_dir = root_b }), 3)
helpers.assert_true(started[3].cmd[3] ~= started[1].cmd[3], "different roots need different data dirs")

local explicit_data_dir = "C:/custom-jdtls-data"
helpers.assert_eq(jdtls.start_or_attach({ root_dir = root_a, data_dir = explicit_data_dir }), 4)
helpers.assert_eq(started[4].cmd[3], explicit_data_dir)

local explicit_cmd = { "custom-jdtls", "--no-data" }
helpers.assert_eq(jdtls.start_or_attach({ root_dir = root_a, cmd = explicit_cmd }), 5)
helpers.assert_eq(started[5].cmd[1], explicit_cmd[1])
helpers.assert_eq(started[5].cmd[2], explicit_cmd[2])
helpers.assert_eq(#started[5].cmd, #explicit_cmd)
helpers.assert_eq(#mkdir_calls, 4)

vim.lsp.start = original_start
jdtls.resolve_extension_jar = original_resolve_extension_jar
vim.fn.executable = original_executable
vim.fn.filereadable = original_filereadable
vim.fn.mkdir = original_mkdir

print("mcdev-nvim jdtls workspace tests passed")
