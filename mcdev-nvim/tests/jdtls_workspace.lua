-- Legacy file ID retained: JDT per-workspace state becomes ktlsp root isolation.
local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local ktlsp = require("mcdev.ktlsp")
local config = require("mcdev.config")
local original_start, original_exec = vim.lsp.start, vim.fn.executable
local original_options = vim.deepcopy(config.options)
local started = {}
vim.lsp.start = function(opts) started[#started + 1] = opts; return #started end
vim.fn.executable = function(path) return path == "ktlsp" and 1 or 0 end
config.options.ktlsp.cmd = { "ktlsp" }
config.options.companion.cmd = { "companion", "--explicit-option" }
local root_a, root_b = "mcdev-workspace-a", "mcdev-workspace-b"
h.assert_eq(ktlsp.start_or_attach({ root_dir = root_a }), 1)
h.assert_eq(started[1].cmd[1], "ktlsp")
h.assert_eq(#started[1].cmd, 1)
h.assert_eq(started[1].root_dir, vim.fs.normalize(vim.fn.fnamemodify(root_a, ":p")):gsub("/+$", ""))
h.assert_eq(ktlsp.start_or_attach({ root_dir = "./" .. root_a }), 2)
h.assert_eq(started[2].root_dir, started[1].root_dir)
h.assert_eq(ktlsp.start_or_attach({ root_dir = root_b }), 3)
h.assert_true(started[3].root_dir ~= started[1].root_dir, "different workspaces must remain separate")
local explicit = { "ktlsp", "--custom-option" }
h.assert_eq(ktlsp.start_or_attach({ root_dir = root_a, cmd = explicit }), 4)
h.assert_eq(started[4].cmd[1], explicit[1])
h.assert_eq(started[4].cmd[2], explicit[2])
h.assert_eq(#started[4].cmd, #explicit)
local provider = started[1].init_options.analysisProviders["mcdev.mixin"]
h.assert_eq(provider.command[1], "companion")
h.assert_eq(provider.command[2], "--explicit-option")
h.assert_eq(provider.enabled, true)
h.assert_eq(vim.tbl_count(provider.configuration), 0)
h.assert_true(started[1].init_options ~= started[2].init_options)
provider.command[1] = "changed"
h.assert_eq(started[2].init_options.analysisProviders["mcdev.mixin"].command[1], "companion")
h.assert_eq(config.options.companion.cmd[1], "companion")
local manual = { cmd = { "custom-host" }, name = "my-ktlsp", init_options = {
  custom = true, analysisProviders = { other = { enabled = false }, ["mcdev.mixin"] = {
    command = { "manual-companion" }, enabled = false, configuration = { preserved = true },
  } },
} }
h.assert_eq(ktlsp.extend_config(manual), manual)
h.assert_eq(manual.name, "my-ktlsp")
h.assert_eq(manual.cmd[1], "custom-host")
h.assert_eq(manual.init_options.custom, true)
h.assert_eq(manual.init_options.analysisProviders.other.enabled, false)
h.assert_eq(manual.init_options.analysisProviders["mcdev.mixin"].command[1], "manual-companion")
h.assert_eq(manual.init_options.analysisProviders["mcdev.mixin"].enabled, false)
h.assert_eq(manual.init_options.analysisProviders["mcdev.mixin"].configuration.preserved, true)
ktlsp.extend_config(manual, { companion = { cmd = { "override" } } })
h.assert_eq(manual.init_options.analysisProviders["mcdev.mixin"].command[1], "override")
config.options.companion.cmd = nil
local missing, err = ktlsp.extend_config({})
h.assert_nil(missing)
h.assert_true(err:find("explicit executable argv", 1, true) ~= nil)
vim.lsp.start, vim.fn.executable = original_start, original_exec
config.options = original_options
print("mcdev-nvim ktlsp workspace and manual configuration tests passed")
