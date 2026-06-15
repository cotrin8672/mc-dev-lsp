local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local buffer = require("mcdev.buffer")

local bundle_jar = vim.env.MCDEV_BUNDLE_JAR
local workspace = vim.env.MCDEV_E2E_WORKSPACE
local jdtls_cmd = vim.env.JDTLS_CMD
local fixture = vim.env.MCDEV_E2E_FIXTURE or "fabric-basic"

helpers.assert_not_nil(bundle_jar, "MCDEV_BUNDLE_JAR is required")
helpers.assert_not_nil(workspace, "MCDEV_E2E_WORKSPACE is required")
helpers.assert_not_nil(jdtls_cmd, "JDTLS_CMD is required")
helpers.assert_true(vim.fn.filereadable(bundle_jar) == 1, "bundle jar must exist: " .. bundle_jar)
helpers.assert_true(vim.fn.isdirectory(workspace) == 1, "workspace must exist: " .. workspace)

local fixture_specs = {
  ["fabric-basic"] = {
    mixin = "src/main/java/com/example/mixin/ExampleMixin.java",
    aw = "src/main/resources/mod.accesswidener",
    at = "src/main/resources/mod_at.cfg",
    platform = "fabric",
    deep_mixin = true,
  },
  ["fabric-mixinextras"] = {
    mixin = "src/main/java/com/example/mixin/MixinExtrasExample.java",
    platform = "fabric",
  },
  ["fabric-aw-at"] = {
    aw = "src/main/resources/mod.accesswidener",
    at = "src/main/resources/mod_at.cfg",
    platform = "fabric",
  },
  ["multi-source-set"] = {
    mixin = "src/main/java/com/example/mixin/MainMixin.java",
    platform = "fabric",
  },
  ["forge-basic"] = {
    mixin = "src/main/java/com/example/mixin/ForgeExampleMixin.java",
    platform = "forge",
  },
}

local fixture_spec = fixture_specs[fixture] or fixture_specs["fabric-basic"]
local mixin_file = fixture_spec.mixin and (workspace .. "/" .. fixture_spec.mixin) or nil
local aw_file = fixture_spec.aw and (workspace .. "/" .. fixture_spec.aw) or nil
local at_file = fixture_spec.at and (workspace .. "/" .. fixture_spec.at) or nil

if mixin_file then
  helpers.assert_true(vim.fn.filereadable(mixin_file) == 1, fixture .. " mixin file must exist: " .. mixin_file)
end
if aw_file then
  helpers.assert_true(vim.fn.filereadable(aw_file) == 1, fixture .. " AW file must exist: " .. aw_file)
end
if at_file then
  helpers.assert_true(vim.fn.filereadable(at_file) == 1, fixture .. " AT file must exist: " .. at_file)
end

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

local function mcdev_command(command, payload, timeout_ms)
  return sync_request("workspace/executeCommand", {
    command = command,
    arguments = { payload },
  }, timeout_ms)
end

local function build_context(bufnr, position)
  return {
    protocolVersion = 1,
    workspaceRoot = workspace_root_uri,
    documentUri = vim.uri_from_bufnr(bufnr),
    languageId = buffer.effective_language_id(bufnr),
    position = {
      line = position[1] - 1,
      character = position[2],
    },
    bufferText = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
    client = {
      name = "mcdev.nvim-e2e",
      version = "0.1.0",
    },
  }
end

local function with_buffer(file_path, filetype, lines, callback)
  local bufnr = vim.fn.bufadd(file_path)
  vim.fn.bufload(bufnr)
  vim.bo[bufnr].filetype = filetype
  if lines then
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  end
  callback(bufnr)
end

sync_request("workspace/executeCommand", {
  command = "java.reloadBundles",
  arguments = { { bundle_jar } },
}, 120000)

if mixin_file then
with_buffer(mixin_file, "java", nil, function(bufnr)
  local buffer_text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  local mixin_line = nil
  for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
    if line:find("@Mixin", 1, true) then
      mixin_line = index - 1
      break
    end
  end
  helpers.assert_not_nil(mixin_line, "@Mixin line not found")

  local context = build_context(bufnr, { mixin_line + 1, 8 })

  local info_result, info_err = mcdev_command("mcdev.info", { context = context })
  helpers.assert_nil(info_err, "mcdev.info failed: " .. vim.inspect(info_err))
  helpers.assert_not_nil(info_result, "mcdev.info returned no payload")

  local info_lines = info_result.result and info_result.result.lines or {}
  helpers.assert_true(#info_lines > 0, "mcdev.info returned no lines")
  local info_text = table.concat(info_lines, "\n")
  helpers.assert_true(
    info_text:lower():find(fixture_spec.platform, 1, true) ~= nil,
    "mcdev.info should report " .. fixture_spec.platform .. " project"
  )
  local completion_result, completion_err = mcdev_command("mcdev.completion", {
    context = context,
    trigger = { kind = "manual" },
    options = {
      preferredAtTarget = "descriptor",
      mixinClassInsert = "import",
      injectMethodDescriptor = "auto",
    },
  })
  helpers.assert_nil(completion_err, "mcdev.completion failed: " .. vim.inspect(completion_err))
  helpers.assert_not_nil(completion_result, "mcdev.completion returned no payload")

  local items = completion_result.result and completion_result.result.items or {}
  helpers.assert_true(#items > 0, "mixin mcdev.completion returned no items")
  helpers.assert_true(items[1].label ~= nil, "completion item must include label")
  helpers.assert_true(items[1].insertText ~= nil, "completion item must include insertText")

  if fixture_spec.deep_mixin then
    local definition_result, definition_err = mcdev_command("mcdev.definition", { context = context })
    helpers.assert_nil(definition_err, "mcdev.definition failed: " .. vim.inspect(definition_err))
    local definition_locations = definition_result.result and definition_result.result.locations or {}
    helpers.assert_true(#definition_locations > 0, "mixin mcdev.definition should return a target")
    helpers.assert_eq(definition_locations[1].metadata.kind, "class")
    helpers.assert_true(
      definition_locations[1].metadata.owner:find("SimpleTarget", 1, true) ~= nil,
      "definition should resolve SimpleTarget"
    )
    helpers.assert_eq(definition_locations[1].resolution, "source")
    helpers.assert_true(
      definition_locations[1].documentUri:find("SimpleTarget.java", 1, true) ~= nil,
      "definition should point to SimpleTarget.java"
    )
    helpers.assert_eq(definition_locations[1].range.start.line, 2)

    local references_result, references_err = mcdev_command("mcdev.references", { context = context })
    helpers.assert_nil(references_err, "mcdev.references failed: " .. vim.inspect(references_err))
    local reference_locations = references_result.result and references_result.result.locations or {}
    helpers.assert_true(#reference_locations > 0, "mixin mcdev.references should return locations")
    helpers.assert_true(
      vim.tbl_filter(function(location)
        return location.metadata and location.metadata.source == "mixin.class"
      end, reference_locations)[1] ~= nil,
      "references should include mixin.class source metadata"
    )

    local inject_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "import org.spongepowered.asm.mixin.injection.Inject;",
      "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class ExampleMixin {",
      "    @Inject(method = \"",
      "    private void mcdev$onDraw(CallbackInfo ci) {}",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, inject_source)
    local inject_line = nil
    for index, line in ipairs(inject_source) do
      if line:find("method = \"", 1, true) then
        inject_line = index
        break
      end
    end
    helpers.assert_not_nil(inject_line, "@Inject method open quote line not found")
    local inject_context = build_context(bufnr, { inject_line, 22 })
    local inject_result, inject_err = mcdev_command("mcdev.completion", {
      context = inject_context,
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "always",
      },
    })
    helpers.assert_nil(inject_err, "mixin inject mcdev.completion failed: " .. vim.inspect(inject_err))
    local inject_items = inject_result.result and inject_result.result.items or {}
    helpers.assert_true(
      vim.tbl_filter(function(item)
        return item.insertText and item.insertText:find("draw%(Ljava/lang/String;FF%)V", 1, false)
      end, inject_items)[1] ~= nil,
      "mixin inject completion should return descriptor-qualified draw overload"
    )
  end
end)
end

if aw_file then
with_buffer(aw_file, "plaintext", {
  "accessWidener v2 named",
  "acc",
}, function(bufnr)
  local context = build_context(bufnr, { 2, 4 })
  local completion_result, completion_err = mcdev_command("mcdev.completion", {
    context = context,
    trigger = { kind = "manual" },
    options = {
      preferredAtTarget = "descriptor",
      mixinClassInsert = "import",
      injectMethodDescriptor = "auto",
    },
  })
  helpers.assert_nil(completion_err, "AW mcdev.completion failed: " .. vim.inspect(completion_err))
  local items = completion_result.result and completion_result.result.items or {}
  helpers.assert_true(
    vim.tbl_filter(function(item)
      return item.label == "accessible" and item.insertText == "accessible"
    end, items)[1] ~= nil,
    "AW completion should include accessible directive"
  )
  helpers.assert_true(
    vim.tbl_filter(function(item)
      return item.metadata and item.metadata.source == "aw.directive"
    end, items)[1] ~= nil,
    "AW completion should tag aw.directive metadata"
  )

  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, {
    "accessWidener v2 named",
    "accessible class com/example/missing/Missing",
  })
  local diagnostics_context = build_context(bufnr, { 2, 21 })
  local diagnostics_result, diagnostics_err = mcdev_command("mcdev.context", { context = diagnostics_context })
  helpers.assert_nil(diagnostics_err, "AW mcdev.context failed: " .. vim.inspect(diagnostics_err))
  local diagnostics = diagnostics_result.result and diagnostics_result.result.diagnostics or {}
  helpers.assert_true(
    vim.tbl_filter(function(diagnostic)
      return diagnostic.code == "AW_UNRESOLVED_CLASS"
    end, diagnostics)[1] ~= nil,
    "AW diagnostics should report unresolved class"
  )
end)
end

if at_file then
with_buffer(at_file, "accesstransformer", {
  "public com.example.target.SimpleTarget draw",
}, function(bufnr)
  local missing_descriptor_context = build_context(bufnr, { 1, 39 })
  local at_diagnostics_result, at_diagnostics_err = mcdev_command("mcdev.context", {
    context = missing_descriptor_context,
  })
  helpers.assert_nil(at_diagnostics_err, "AT mcdev.context failed: " .. vim.inspect(at_diagnostics_err))
  local at_diagnostics = at_diagnostics_result.result and at_diagnostics_result.result.diagnostics or {}
  local missing_descriptor = vim.tbl_filter(function(diagnostic)
    return diagnostic.code == "AT_MISSING_METHOD_DESCRIPTOR"
  end, at_diagnostics)[1]
  if missing_descriptor then
    local code_action_result, code_action_err = mcdev_command("mcdev.codeAction", {
      context = missing_descriptor_context,
      range = missing_descriptor.range,
      diagnosticCodes = { missing_descriptor.code },
    })
    helpers.assert_nil(code_action_err, "AT mcdev.codeAction failed: " .. vim.inspect(code_action_err))
    local actions = code_action_result.result and code_action_result.result.actions or {}
    helpers.assert_true(#actions > 0, "AT code action should offer a descriptor fix")
    helpers.assert_true(
      actions[1].kind == "quickfix.at.addDescriptor" or actions[1].title:lower():find("descriptor", 1, true) ~= nil,
      "AT code action should add a method descriptor"
    )
  end
end)
end

with_buffer(at_file, "accesstransformer", {
  "public com.example.target.SimpleTarget dr",
}, function(bufnr)
  local context = build_context(bufnr, { 1, 39 })
  local completion_result, completion_err = mcdev_command("mcdev.completion", {
    context = context,
    trigger = { kind = "manual" },
    options = {
      preferredAtTarget = "descriptor",
      mixinClassInsert = "import",
      injectMethodDescriptor = "auto",
    },
  })
  helpers.assert_nil(completion_err, "AT mcdev.completion failed: " .. vim.inspect(completion_err))
  local items = completion_result.result and completion_result.result.items or {}
  local method_item = vim.tbl_filter(function(item)
    return item.insertText == "method_1(Ljava/lang/String;FF)V"
      or item.insertText == "draw(Ljava/lang/String;FF)V"
  end, items)[1]
  helpers.assert_not_nil(method_item, "AT completion should return mapped method insert text")
  helpers.assert_true(
    method_item.metadata and method_item.metadata.source == "at.member.method",
    "AT completion should tag at.member.method metadata"
  )
end)

print("mcdev osgi bundle e2e passed for " .. fixture)
vim.cmd("qa!")
