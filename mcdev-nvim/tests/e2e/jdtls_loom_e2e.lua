local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local buffer = require("mcdev.buffer")

local bundle_jar = vim.env.MCDEV_BUNDLE_JAR
local workspace = vim.env.MCDEV_E2E_WORKSPACE
local jdtls_cmd = vim.env.JDTLS_CMD

helpers.assert_not_nil(bundle_jar, "MCDEV_BUNDLE_JAR is required")
helpers.assert_not_nil(workspace, "MCDEV_E2E_WORKSPACE is required")
helpers.assert_not_nil(jdtls_cmd, "JDTLS_CMD is required")
helpers.assert_true(vim.fn.filereadable(bundle_jar) == 1, "bundle jar must exist: " .. bundle_jar)
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

local data_dir = vim.fn.stdpath("cache") .. "/mcdev-osgi-loom-e2e-jdtls"
vim.fn.mkdir(data_dir, "p")

local workspace_root_uri = workspace_uri(workspace)

local client_id = vim.lsp.start({
  name = "jdtls",
  cmd = { jdtls_cmd, "-data", data_dir },
  root_dir = workspace,
  workspace_folders = {
    {
      uri = workspace_root_uri,
      name = "mcdev-loom-e2e-workspace",
    },
  },
  init_options = {
    bundles = { bundle_jar },
    settings = {
      java = {
        import = {
          gradle = { enabled = false },
          maven = { enabled = false },
        },
        configuration = {
          updateBuildConfiguration = "disabled",
        },
      },
    },
  },
  capabilities = vim.lsp.protocol.make_client_capabilities(),
  flags = {
    debounce_text_changes = 0,
  },
})

helpers.assert_not_nil(client_id, "failed to start jdtls client")

local initialized = vim.wait(240000, function()
  local client = vim.lsp.get_client_by_id(client_id)
  return client ~= nil and client.initialized == true
end, 250)

if not initialized then
  local client = vim.lsp.get_client_by_id(client_id)
  error("jdtls client did not initialize within 240s: " .. vim.inspect({
    client_exists = client ~= nil,
    initialized = client and client.initialized or false,
  }))
end

local client = vim.lsp.get_client_by_id(client_id)
helpers.assert_not_nil(client, "jdtls client disappeared after initialize")

local function sync_request(method, params, timeout_ms)
  local result = nil
  local err = nil
  client:request(method, params, function(request_err, request_result)
    err = request_err
    result = request_result
  end, timeout_ms or 120000)
  local completed = vim.wait(timeout_ms or 120000, function()
    return result ~= nil or err ~= nil
  end, 100)
  helpers.assert_true(completed, method .. " timed out")
  return result, err
end

local function mcdev_command(command, payload, timeout_ms)
  return sync_request("workspace/executeCommand", {
    command = command,
    arguments = { payload },
  }, timeout_ms)
end

sync_request("workspace/executeCommand", {
  command = "java.reloadBundles",
  arguments = { { bundle_jar } },
}, 120000)

vim.wait(60000, function()
  return #vim.lsp.get_clients({ name = "jdtls" }) > 0
end, 250)

local bufnr = vim.fn.bufadd(mixin_file)
vim.fn.bufload(bufnr)
vim.bo[bufnr].filetype = "java"

local mixin_line = nil
for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
  if line:find("@Mixin", 1, true) then
    mixin_line = index - 1
    break
  end
end
helpers.assert_not_nil(mixin_line, "@Mixin line not found")

local context = {
  protocolVersion = 1,
  workspaceRoot = workspace_root_uri,
  documentUri = vim.uri_from_bufnr(bufnr),
  languageId = buffer.effective_language_id(bufnr),
  position = { line = mixin_line, character = 8 },
  bufferText = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
  client = { name = "mcdev.nvim-loom-e2e", version = "0.1.0" },
}

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
