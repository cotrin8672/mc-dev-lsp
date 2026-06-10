local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")

local bundle_jar = vim.env.MCDEV_BUNDLE_JAR
local workspace = vim.env.MCDEV_E2E_WORKSPACE
local jdtls_cmd = vim.env.JDTLS_CMD

helpers.assert_not_nil(bundle_jar, "MCDEV_BUNDLE_JAR is required")
helpers.assert_not_nil(workspace, "MCDEV_E2E_WORKSPACE is required")
helpers.assert_not_nil(jdtls_cmd, "JDTLS_CMD is required")
helpers.assert_true(vim.fn.filereadable(bundle_jar) == 1, "bundle jar must exist: " .. bundle_jar)
helpers.assert_true(vim.fn.isdirectory(workspace) == 1, "workspace must exist: " .. workspace)

local mixin_file = workspace .. "/src/main/java/com/example/mixin/ExampleMixin.java"
helpers.assert_true(vim.fn.filereadable(mixin_file) == 1, "ExampleMixin.java must exist")

local function workspace_uri(path)
  return vim.uri_from_fname(path)
end

local data_dir = vim.fn.stdpath("cache") .. "/mcdev-osgi-e2e-jdtls"
vim.fn.mkdir(data_dir, "p")

local workspace_root_uri = workspace_uri(workspace)

local client_id = vim.lsp.start({
  name = "jdtls",
  cmd = { jdtls_cmd, "-data", data_dir },
  root_dir = workspace,
  workspace_folders = {
    {
      uri = workspace_root_uri,
      name = "mcdev-e2e-workspace",
    },
  },
  init_options = {
    bundles = { bundle_jar },
  },
  capabilities = vim.lsp.protocol.make_client_capabilities(),
  flags = {
    debounce_text_changes = 0,
  },
})

helpers.assert_not_nil(client_id, "failed to start jdtls client")

local initialized = vim.wait(180000, function()
  local client = vim.lsp.get_client_by_id(client_id)
  return client ~= nil and client.initialized == true
end, 250)

if not initialized then
  local client = vim.lsp.get_client_by_id(client_id)
  error("jdtls client did not initialize within 180s: " .. vim.inspect({
    client_exists = client ~= nil,
    initialized = client and client.initialized or false,
    is_stopped = client and client.is_stopped(client) or true,
  }))
end

local client = vim.lsp.get_client_by_id(client_id)
helpers.assert_not_nil(client, "jdtls client disappeared after initialize")

local buf = vim.fn.bufadd(mixin_file)
vim.fn.bufload(buf)
vim.bo[buf].filetype = "java"

local buffer_text = table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n")
local mixin_line = nil
for index, line in ipairs(vim.api.nvim_buf_get_lines(buf, 0, -1, false)) do
  if line:find("@Mixin", 1, true) then
    mixin_line = index - 1
    break
  end
end
helpers.assert_not_nil(mixin_line, "@Mixin line not found")

local context = {
  protocolVersion = 1,
  workspaceRoot = workspace_root_uri,
  documentUri = vim.uri_from_bufnr(buf),
  languageId = "java",
  position = {
    line = mixin_line,
    character = 7,
  },
  bufferText = buffer_text,
  client = {
    name = "mcdev.nvim-e2e",
    version = "0.1.0",
  },
}

local function sync_request(method, params, timeout_ms)
  local result = nil
  local err = nil
  client:request(method, params, function(request_err, request_result)
    err = request_err
    result = request_result
  end, timeout_ms or 60000)
  local completed = vim.wait(timeout_ms or 60000, function()
    return result ~= nil or err ~= nil
  end, 100)
  helpers.assert_true(completed, method .. " timed out")
  return result, err
end

sync_request("workspace/executeCommand", {
  command = "java.reloadBundles",
  arguments = { { bundle_jar } },
}, 120000)

local info_result, info_err = sync_request("workspace/executeCommand", {
  command = "mcdev.info",
  arguments = { { context = context } },
})
helpers.assert_nil(info_err, "mcdev.info failed: " .. vim.inspect(info_err))
helpers.assert_not_nil(info_result, "mcdev.info returned no payload")

local info_lines = info_result.result and info_result.result.lines or {}
helpers.assert_true(#info_lines > 0, "mcdev.info returned no lines")
local info_text = table.concat(info_lines, "\n")
helpers.assert_true(info_text:lower():find("fabric", 1, true) ~= nil, "mcdev.info should report Fabric project")

local completion_result, completion_err = sync_request("workspace/executeCommand", {
  command = "mcdev.completion",
  arguments = {
    {
      context = context,
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    },
  },
})
helpers.assert_nil(completion_err, "mcdev.completion failed: " .. vim.inspect(completion_err))
helpers.assert_not_nil(completion_result, "mcdev.completion returned no payload")

local items = completion_result.result and completion_result.result.items or {}
helpers.assert_true(#items > 0, "mcdev.completion returned no items")
helpers.assert_true(items[1].label ~= nil, "completion item must include label")
helpers.assert_true(items[1].insertText ~= nil, "completion item must include insertText")

print("mcdev osgi bundle e2e passed")
vim.cmd("qa!")
