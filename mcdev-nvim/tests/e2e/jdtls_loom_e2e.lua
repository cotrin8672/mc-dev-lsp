local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local protocol = require("mcdev.protocol")

local companion_cmd = vim.env.MCDEV_E2E_COMPANION
local workspace = vim.env.MCDEV_E2E_WORKSPACE
local ktlsp_cmd = vim.env.MCDEV_E2E_KTLSP

helpers.assert_not_nil(companion_cmd, "MCDEV_E2E_COMPANION is required")
helpers.assert_not_nil(workspace, "MCDEV_E2E_WORKSPACE is required")
helpers.assert_not_nil(ktlsp_cmd, "MCDEV_E2E_KTLSP is required")
helpers.assert_true(vim.fn.executable(companion_cmd) == 1, "companion executable must exist: " .. companion_cmd)
helpers.assert_true(vim.fn.executable(ktlsp_cmd) == 1, "ktlsp executable must exist: " .. ktlsp_cmd)
helpers.assert_true(vim.fn.isdirectory(workspace) == 1, "workspace must exist: " .. workspace)

local mixin_file = workspace .. "/src/main/java/com/example/mixin/ExampleMixin.java"
local mapped_source = workspace .. "/mapped-sources/com/example/target/SimpleTarget.java"
local mod_source = workspace .. "/src/main/java/com/example/target/SimpleTarget.java"

helpers.assert_true(vim.fn.filereadable(mixin_file) == 1, "ExampleMixin.java must exist")
helpers.assert_true(vim.fn.filereadable(mapped_source) == 1, "mapped SimpleTarget.java must exist")
helpers.assert_true(vim.fn.filereadable(mod_source) == 0, "SimpleTarget must not live in mod sources")

local function workspace_uri(path)
  return vim.uri_from_fname(path)
end

local workspace_root_uri = workspace_uri(workspace)
local bufnr = vim.fn.bufadd(mixin_file)
vim.fn.bufload(bufnr)
vim.api.nvim_set_current_buf(bufnr)
vim.bo[bufnr].filetype = "java"

require("mcdev").setup({ ktlsp = { cmd = { ktlsp_cmd } }, companion = { cmd = { companion_cmd } } })
local client_id = require("mcdev.ktlsp").start_or_attach({
  root_dir = workspace,
  workspace_folders = { { uri = workspace_root_uri, name = "mcdev-loom-e2e-workspace" } },
  flags = { debounce_text_changes = 0 },
})

helpers.assert_not_nil(client_id, "failed to start ktlsp client")

local initialized = vim.wait(240000, function()
  local client = vim.lsp.get_client_by_id(client_id)
  return client ~= nil and client.initialized == true
end, 250)

if not initialized then
  local client = vim.lsp.get_client_by_id(client_id)
  error("ktlsp client did not initialize within 240s: " .. vim.inspect({
    client_exists = client ~= nil,
    initialized = client and client.initialized or false,
  }))
end

local client = vim.lsp.get_client_by_id(client_id)
helpers.assert_not_nil(client, "ktlsp client disappeared after initialize")
helpers.assert_true(vim.lsp.buf_attach_client(bufnr, client_id), "failed to attach ktlsp to the mixin document")

local function mcdev_command(command, payload, timeout_ms)
  local result, err, completed
  local cancel = protocol.request(command, payload, function(request_result, request_err)
    result, err, completed = request_result, request_err, true
  end, bufnr)
  local settled = vim.wait(timeout_ms or 120000, function() return completed end, 100)
  if not settled then cancel() end
  helpers.assert_true(settled, command .. " timed out")
  return result, err
end

mcdev_command("mcdev.reloadProjectContext", { context = protocol.context(bufnr, { 1, 0 }) }, 120000)

vim.wait(60000, function()
  return #vim.lsp.get_clients({ name = "ktlsp" }) > 0
end, 250)

local mixin_line = nil
for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
  if line:find("@Mixin", 1, true) then
    mixin_line = index - 1
    break
  end
end
helpers.assert_not_nil(mixin_line, "@Mixin line not found")

local context = protocol.context(bufnr, { mixin_line + 1, 8 })
context.client.name = "mcdev.nvim-loom-e2e"

mcdev_command("mcdev.reindex", { context = context }, 120000)

local info_result, info_err = mcdev_command("mcdev.info", { context = context }, 120000)
helpers.assert_nil(info_err, "mcdev.info failed: " .. vim.inspect(info_err))
local info_text = table.concat((info_result.result and info_result.result.lines) or {}, "\n")
helpers.assert_true(info_text:lower():find("fabric", 1, true) ~= nil, "mcdev.info should report Fabric project")

local definition_result, definition_err = mcdev_command("mcdev.definition", { context = context }, 120000)
helpers.assert_nil(definition_err, "mcdev.definition failed: " .. vim.inspect(definition_err))
local definition_locations = definition_result.result and definition_result.result.locations or {}
helpers.assert_true(#definition_locations > 0, "loom mcdev.definition should return a target")

local location = definition_locations[1]
helpers.assert_eq(location.metadata.kind, "class")
helpers.assert_true(
  location.metadata.owner:find("SimpleTarget", 1, true) ~= nil,
  "definition should resolve SimpleTarget"
)

local resolution = location.resolution
helpers.assert_true(
  resolution == "jdt" or resolution == "source",
  "expected jdt or source resolution, got: " .. vim.inspect(resolution)
)

if resolution == "jdt" then
  helpers.assert_true(
    location.documentUri:find("mapped-sources", 1, true) ~= nil,
    "jdt definition should point at mapped-sources: " .. location.documentUri
  )
  helpers.assert_eq(location.range.start.line, 3)
else
  helpers.assert_true(
    location.documentUri:find("SimpleTarget.java", 1, true) ~= nil,
    "source definition should point at SimpleTarget.java, got: " .. vim.inspect(location.documentUri)
  )
  helpers.assert_true(
    location.documentUri:find("mapped-sources", 1, true) ~= nil,
    "source definition should prefer mapped-sources: " .. location.documentUri
  )
end

print("mcdev loom osgi e2e passed (resolution=" .. tostring(resolution) .. ")")
vim.cmd("qa!")
