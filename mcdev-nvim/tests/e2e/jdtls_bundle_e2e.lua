local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local buffer = require("mcdev.buffer")
local mcdev = require("mcdev")
local convert = require("mcdev.convert")
local completion = require("mcdev.completion")
local health = require("mcdev.health")

local bundle_jar = vim.env.MCDEV_BUNDLE_JAR
local workspace = vim.env.MCDEV_E2E_WORKSPACE
local jdtls_cmd = vim.env.JDTLS_CMD
local fixture = vim.env.MCDEV_E2E_FIXTURE or "fabric-basic"
local progress_log = vim.fn.getcwd() .. "/build/e2e-progress.log"
local health_log = vim.fn.getcwd() .. "/build/e2e-health.log"
local debug_completion_log = vim.fn.getcwd() .. "/build/e2e-debug-completion.log"

local function log_step(message)
  vim.fn.mkdir(vim.fn.fnamemodify(progress_log, ":h"), "p")
  vim.fn.writefile({ os.date("%Y-%m-%d %H:%M:%S ") .. message }, progress_log, "a")
end

vim.fn.writefile({}, progress_log)
vim.fn.writefile({}, health_log)
vim.fn.writefile({}, debug_completion_log)
log_step("starting osgi bundle e2e for " .. fixture)

helpers.assert_not_nil(bundle_jar, "MCDEV_BUNDLE_JAR is required")
helpers.assert_not_nil(workspace, "MCDEV_E2E_WORKSPACE is required")
helpers.assert_not_nil(jdtls_cmd, "JDTLS_CMD is required")
helpers.assert_true(vim.fn.filereadable(bundle_jar) == 1, "bundle jar must exist: " .. bundle_jar)
helpers.assert_true(vim.fn.isdirectory(workspace) == 1, "workspace must exist: " .. workspace)
log_step("environment validated")

local fixture_specs = {
  ["real-cem"] = {
    mixin = "src/main/java/io/github/cotrin8672/cem/mixin/ItemMixin.java",
  },
  ["real-sodium"] = {
    mixin = "common/src/main/java/net/caffeinemc/mods/sodium/mixin/features/options/overlays/GuiMixin.java",
  },
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
    client_mixin = "src/client/java/com/example/mixin/ClientMixin.java",
    platform = "fabric",
  },
  ["forge-basic"] = {
    mixin = "src/main/java/com/example/mixin/ForgeExampleMixin.java",
    platform = "forge",
  },
}

local fixture_spec = fixture_specs[fixture] or fixture_specs["fabric-basic"]
local mixin_file = fixture_spec.mixin and (workspace .. "/" .. fixture_spec.mixin) or nil
local client_mixin_file = fixture_spec.client_mixin and (workspace .. "/" .. fixture_spec.client_mixin) or nil
local aw_file = fixture_spec.aw and (workspace .. "/" .. fixture_spec.aw) or nil
local at_file = fixture_spec.at and (workspace .. "/" .. fixture_spec.at) or nil
log_step("fixture paths built")

if mixin_file then
  helpers.assert_true(vim.fn.filereadable(mixin_file) == 1, fixture .. " mixin file must exist: " .. mixin_file)
end
if client_mixin_file then
  helpers.assert_true(
    vim.fn.filereadable(client_mixin_file) == 1,
    fixture .. " client mixin file must exist: " .. client_mixin_file
  )
end
if aw_file then
  helpers.assert_true(vim.fn.filereadable(aw_file) == 1, fixture .. " AW file must exist: " .. aw_file)
end
if at_file then
  helpers.assert_true(vim.fn.filereadable(at_file) == 1, fixture .. " AT file must exist: " .. at_file)
end
log_step("fixture files validated")

local function workspace_uri(path)
  return vim.uri_from_fname(path)
end

local data_dir = vim.fn.stdpath("cache") .. "/mcdev-osgi-e2e-jdtls"
local preserve_data = vim.env.MCDEV_E2E_KEEP_JDTLS_DATA == "1"
if not preserve_data then
  vim.fn.delete(data_dir, "rf")
end
vim.fn.mkdir(data_dir, "p")
log_step(preserve_data and "data dir prepared (preserved)" or "data dir prepared (cold/reset)")
local bundle_snapshot = data_dir .. "/mcdev-bundle-" .. vim.fn.getpid() .. ".jar"
local copied, copy_error = vim.uv.fs_copyfile(bundle_jar, bundle_snapshot)
helpers.assert_true(copied == true, "cannot snapshot E2E bundle: " .. tostring(copy_error))
bundle_jar = bundle_snapshot
log_step("bundle snapshot: " .. bundle_jar)

local workspace_root_uri = workspace_uri(workspace)
log_step("workspace uri built")

local function first_glob(pattern)
  local matches = vim.fn.glob(pattern, false, true)
  return matches and matches[1] or nil
end

local function mason_jdtls_base_from_cmd(cmd)
  local normalized = cmd:gsub("\\", "/")
  local mason_bin = "/mason/bin/jdtls.cmd"
  if normalized:lower():sub(-#mason_bin) ~= mason_bin then
    return nil
  end
  return normalized:sub(1, #normalized - #mason_bin) .. "/mason/packages/jdtls"
end

local function direct_java_jdtls_cmd(cmd)
  if vim.fn.has("win32") ~= 1 or cmd:lower():sub(-4) ~= ".cmd" then
    return nil
  end
  local base = mason_jdtls_base_from_cmd(cmd)
  if not base then
    return nil
  end
  local launcher = first_glob(base .. "/plugins/org.eclipse.equinox.launcher_*.jar")
    or first_glob(base .. "/plugins/org.eclipse.equinox.launcher.jar")
  if not launcher then
    return nil
  end
  return {
    "java",
    "-Declipse.application=org.eclipse.jdt.ls.core.id1",
    "-Dosgi.bundles.defaultStartLevel=4",
    "-Declipse.product=org.eclipse.jdt.ls.core.product",
    "-Dosgi.checkConfiguration=true",
    "-Dosgi.sharedConfiguration.area=" .. base .. "/config_win",
    "-Dosgi.sharedConfiguration.area.readOnly=true",
    "-Dosgi.configuration.cascaded=true",
    "-Xms1G",
    "--add-modules=ALL-SYSTEM",
    "--add-opens",
    "java.base/java.util=ALL-UNNAMED",
    "--add-opens",
    "java.base/java.lang=ALL-UNNAMED",
    "-jar",
    launcher,
    "-data",
    data_dir,
  }
end
local jdtls_launch_cmd = direct_java_jdtls_cmd(jdtls_cmd) or { jdtls_cmd, "-data", data_dir }
if vim.fn.has("win32") == 1 and jdtls_launch_cmd[1]:lower():sub(-4) == ".cmd" then
  jdtls_launch_cmd = { "cmd.exe", "/C", (jdtls_cmd:gsub("\\", "/")), "-data", (data_dir:gsub("\\", "/")) }
end
log_step("jdtls launch cmd: " .. table.concat(jdtls_launch_cmd, " "))

local jdtls_exit = { exited = false, code = nil, signal = nil }
_G.mcdev_e2e_jdtls_exit = jdtls_exit
local client_id = vim.lsp.start_client({
  name = "jdtls",
  cmd = jdtls_launch_cmd,
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
  settings = vim.env.MCDEV_E2E_GRADLE_JAVA_HOME and {
    java = { import = { gradle = { java = { home = vim.env.MCDEV_E2E_GRADLE_JAVA_HOME } } } },
  } or nil,
  capabilities = vim.lsp.protocol.make_client_capabilities(),
  flags = {
    debounce_text_changes = 0,
  },
  on_exit = function(code, signal)
    jdtls_exit.exited = true
    jdtls_exit.code = code
    jdtls_exit.signal = signal
  end,
})

helpers.assert_not_nil(client_id, "failed to start jdtls client")
log_step("started jdtls client " .. tostring(client_id))

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
log_step("jdtls initialized")

mcdev.setup({
  jdtls = {
    extension_jar = bundle_jar,
  },
})

local function sync_request(method, params, timeout_ms)
  log_step("request " .. method)
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
  log_step("response " .. method .. " err=" .. tostring(err ~= nil) .. " result=" .. tostring(result ~= nil))
  if err ~= nil then
    log_step("error " .. method .. " " .. vim.inspect(err))
  end
  return result, err
end

local function mcdev_command(command, payload, timeout_ms)
  log_step("command " .. command)
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

local function compile_single_java_source(source_path)
  local gradle_cache = vim.fn.expand("~/.gradle/caches/modules-2/files-2.1")
  local mixin_jar = first_glob(gradle_cache .. "/org.spongepowered/mixin/0.8.7/*/mixin-0.8.7.jar")
  local mixinextras_jar = first_glob(
    gradle_cache .. "/io.github.llamalad7/mixinextras-fabric/0.5.5/*/mixinextras-fabric-0.5.5.jar"
  )
  helpers.assert_not_nil(mixin_jar, "Mixin 0.8.7 jar is required for javac E2E verification")
  helpers.assert_not_nil(mixinextras_jar, "MixinExtras Fabric 0.5.5 jar is required for javac E2E verification")

  local output_dir = workspace .. "/build/mcdev-e2e-compile"
  vim.fn.delete(output_dir, "rf")
  vim.fn.mkdir(output_dir, "p")
  local path_separator = package.config:sub(1, 1) == "\\" and ";" or ":"
  local classpath = table.concat({ workspace .. "/classpath", mixin_jar, mixinextras_jar }, path_separator)
  local output = vim.fn.systemlist({
    "javac",
    "-proc:none",
    "-encoding",
    "UTF-8",
    "-classpath",
    classpath,
    "-d",
    output_dir,
    source_path,
  })
  helpers.assert_eq(
    vim.v.shell_error,
    0,
    "javac failed for " .. source_path .. ": " .. table.concat(output or {}, "\n")
  )
end

local function with_buffer(file_path, filetype, lines, callback)
  local bufnr = vim.fn.bufadd(file_path)
  vim.fn.bufload(bufnr)
  vim.api.nvim_set_current_buf(bufnr)
  vim.bo[bufnr].filetype = filetype
  if lines then
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  end
  vim.lsp.buf_attach_client(bufnr, client_id)
  vim.wait(1000, function()
    return vim.lsp.get_clients({ bufnr = bufnr, name = "jdtls" })[1] ~= nil
  end, 50)
  callback(bufnr)
end

local function find_line_and_character(lines, marker)
  for index, line in ipairs(lines) do
    local marker_start = line:find(marker, 1, true)
    if marker_start then
      return index, marker_start - 1
    end
  end
  return nil, nil
end

local function capture_notify(callback, timeout_ms)
  local original_notify = vim.notify
  local message = nil
  vim.notify = function(text)
    message = tostring(text)
  end
  callback()
  vim.wait(timeout_ms or 60000, function()
    return message ~= nil
  end, 100)
  vim.notify = original_notify
  helpers.assert_not_nil(message, "expected notification output")
  return message
end

log_step("bundle provided through init_options.bundles")

if fixture == "real-cem" then
  dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/cem_project_e2e.lua")({
    helpers = helpers, with_buffer = with_buffer, mixin_file = mixin_file, log_step = log_step,
  })
  return
end

if fixture == "real-sodium" then
  dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/real_project_e2e.lua")({
    helpers = helpers,
    with_buffer = with_buffer,
    mixin_file = mixin_file,
    log_step = log_step,
  })
  log_step("passed real project completion correctness; latency recorded separately")
  return
end

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
  log_step("request workspaceRoot: " .. tostring(context.workspaceRoot))
  log_step("request documentUri: " .. tostring(context.documentUri))

  if fixture == "fabric-basic" then
    local at_line, at_character
    for index, line in ipairs(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)) do
      local at_start = line:find('@At("HEAD")', 1, true)
      if at_start then
        at_line, at_character = index, at_start + #('@At(')
        break
      end
    end
    helpers.assert_not_nil(at_line, "@At(\"HEAD\") line not found")
    local before_lines = vim.api.nvim_buf_get_lines(bufnr, 0, -1, false)
    vim.api.nvim_buf_set_lines(bufnr, at_line - 1, at_line, false, {
      (before_lines[at_line]:gsub('@At%("HEAD"%)', '@At("HE")')),
    })
    at_character = at_character + 2
    local stream_started = vim.uv.hrtime()
    local first_response_ms
    local stream_results = {}
    local stream_settled = false
    local stream_cancel = completion.complete(function(result)
      first_response_ms = first_response_ms or (vim.uv.hrtime() - stream_started) / 1e6
      stream_results[#stream_results + 1] = result
      if not result.isProvisional then
        stream_settled = true
      end
    end, bufnr, { at_line, at_character }, { source = "e2e-cold-stream", stream = true })
    local stream_waited = vim.wait(60000, function()
      return stream_settled
    end, 100)
    if not stream_waited then
      stream_cancel()
    end
    helpers.assert_true(stream_waited, "cold completion stream did not settle")
    local stream_elapsed_ms = (vim.uv.hrtime() - stream_started) / 1e6
    local first_result = stream_results[1] or {}
    local final_result = stream_results[#stream_results] or {}
    local first_head = vim.tbl_filter(function(item)
      return item.insertText == "HEAD"
    end, first_result.items or {})[1]
    local final_head = vim.tbl_filter(function(item)
      return item.insertText == "HEAD"
    end, final_result.items or {})[1]
    log_step(string.format(
      "cold @At adapter stream elapsed_ms=%.1f callbacks=%d provisional_first=%s helper_requests=%d first_response_ms=%.1f",
      stream_elapsed_ms,
      #stream_results,
      tostring(first_result.isProvisional == true),
      completion.helper_request_count,
      first_response_ms or -1
    ))
    helpers.assert_true(first_head ~= nil or final_head ~= nil, "cold adapter completion should include HEAD")
    helpers.assert_true(
      first_result.isProvisional == true or #stream_results == 1,
      "cold adapter completion should settle directly or deliver a provisional result first"
    )
    local applied_item = final_head or first_head
    helpers.assert_not_nil(applied_item.textEdit, "cold adapter HEAD completion should include a text edit")
    vim.lsp.util.apply_text_edits({ applied_item.textEdit }, bufnr, "utf-16")
    helpers.assert_true(
      vim.api.nvim_buf_get_lines(bufnr, at_line - 1, at_line, false)[1]:find('@At("HEAD")', 1, true) ~= nil,
      "cold adapter HEAD edit should complete the partial value"
    )
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, before_lines)
  end

  local info_result, info_err = mcdev_command("mcdev.info", { context = context })
  helpers.assert_nil(info_err, "mcdev.info failed: " .. vim.inspect(info_err))
  helpers.assert_not_nil(info_result, "mcdev.info returned no payload")

  local info_lines = info_result.result and info_result.result.lines or {}
  helpers.assert_true(#info_lines > 0, "mcdev.info returned no lines")
  helpers.assert_not_nil(info_result.result.buildCommit, "mcdev.info should include extension build commit")
  helpers.assert_not_nil(info_result.result.buildTime, "mcdev.info should include extension build time")
  helpers.assert_not_nil(info_result.result.jarLocation, "mcdev.info should include extension jar location")
  helpers.assert_true(
    vim.tbl_contains(info_result.result.registeredCommands or {}, "mcdev.completion"),
    "mcdev.info should include registered mcdev.completion command"
  )
  local info_text = table.concat(info_lines, "\n")
  helpers.assert_true(
    info_text:find("Extension loaded: true", 1, true) ~= nil,
    "mcdev.info should report extension loaded"
  )
  helpers.assert_true(
    info_text:find("Extension build commit:", 1, true) ~= nil,
    "mcdev.info should report extension build commit line"
  )
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
  local debug = completion_result.result and completion_result.result.debug or {}
  helpers.assert_true(
    #items > 0,
    string.format(
      "mixin mcdev.completion returned no items (parseSource=%s, completionContextKind=%s, candidateCountBeforeFilter=%s, candidateCountAfterFilter=%s, warnings=%s)",
      tostring(debug.parseSource),
      tostring(debug.completionContextKind),
      tostring(debug.candidateCountBeforeFilter),
      tostring(debug.candidateCountAfterFilter),
      vim.inspect(debug.warnings or {})
    )
  )
  helpers.assert_true(items[1].label ~= nil, "completion item must include label")
  helpers.assert_true(items[1].insertText ~= nil, "completion item must include insertText")
  log_step("completion debug: " .. vim.inspect(debug))
  helpers.assert_eq(debug.command, "mcdev.completion")
  helpers.assert_eq(debug.zeroItemReason, nil)
  helpers.assert_eq(debug.parseSource, "JDT_AST")
  helpers.assert_eq(debug.usedCompilationUnit, true)
  helpers.assert_eq(debug.usedJavaProject, true)
  helpers.assert_true((debug.bindingResolvedCount or 0) > 0, "completion debug should resolve JDT bindings")
  helpers.assert_eq(debug.semanticContextFound, true)
  helpers.assert_eq(debug.fallbackAnnotationContextUsed, false)
  helpers.assert_eq(#(debug.warnings or {}), 0)
  helpers.assert_true((debug.semanticTargetCount or 0) > 0, "completion debug should report semantic targets")

  if fixture == "multi-source-set" then
    local main_target_item = vim.tbl_filter(function(item)
      return item.label == "SimpleTarget" or item.insertText == "SimpleTarget"
    end, items)[1]
    helpers.assert_not_nil(main_target_item, "main source-set completion should return SimpleTarget from classpath")

    with_buffer(client_mixin_file, "java", nil, function(client_bufnr)
      local client_mixin_line = nil
      for index, line in ipairs(vim.api.nvim_buf_get_lines(client_bufnr, 0, -1, false)) do
        if line:find("@Mixin", 1, true) then
          client_mixin_line = index - 1
          break
        end
      end
      helpers.assert_not_nil(client_mixin_line, "client @Mixin line not found")

      local client_started = vim.uv.hrtime()
      local client_result, client_err = mcdev_command("mcdev.completion", {
        context = build_context(client_bufnr, { client_mixin_line + 1, 8 }),
        trigger = { kind = "manual" },
        options = {
          preferredAtTarget = "descriptor",
          mixinClassInsert = "import",
          injectMethodDescriptor = "auto",
        },
      })
      local client_elapsed_ms = (vim.uv.hrtime() - client_started) / 1e6
      helpers.assert_nil(client_err, "client source-set mcdev.completion failed: " .. vim.inspect(client_err))
      helpers.assert_not_nil(client_result, "client source-set mcdev.completion returned no payload")

      local client_items = client_result.result and client_result.result.items or {}
      local client_debug = client_result.result and client_result.result.debug or {}
      log_step(string.format(
        "multi-source-set client completion elapsed_ms=%.1f items=%d parseSource=%s",
        client_elapsed_ms,
        #client_items,
        tostring(client_debug.parseSource)
      ))
      helpers.assert_not_nil(
        vim.tbl_filter(function(item)
          return item.label == "SimpleTarget" or item.insertText == "SimpleTarget"
        end, client_items)[1],
        "client source-set completion should return SimpleTarget from classpath"
      )
      helpers.assert_eq(client_debug.parseSource, "JDT_AST")
      helpers.assert_eq(client_debug.usedCompilationUnit, true)
      helpers.assert_eq(client_debug.usedJavaProject, true)
      helpers.assert_eq(client_debug.semanticContextFound, true)
      helpers.assert_eq(#(client_debug.warnings or {}), 0)
    end)
    vim.api.nvim_set_current_buf(bufnr)
  end

  vim.api.nvim_win_set_cursor(0, { mixin_line + 1, 8 })
  local health_output = capture_notify(function()
    health.health(bufnr)
  end, 60000)
  vim.fn.writefile(vim.split(health_output, "\n", { plain = true }), health_log)
  helpers.assert_true(health_output:find("mcdev.info ping: OK", 1, true) ~= nil, health_output)
  helpers.assert_true(health_output:find("mcdev.completion ping: OK", 1, true) ~= nil, health_output)
  helpers.assert_true(health_output:find("extension build commit:", 1, true) ~= nil, health_output)
  helpers.assert_true(health_output:find("usedCompilationUnit: true", 1, true) ~= nil, health_output)
  helpers.assert_true(health_output:find("fallbackAnnotationContextUsed: false", 1, true) ~= nil, health_output)

  local debug_completion_output = capture_notify(function()
    health.debug_completion(bufnr)
  end, 60000)
  vim.fn.writefile(vim.split(debug_completion_output, "\n", { plain = true }), debug_completion_log)
  helpers.assert_true(debug_completion_output:find("parse source: JDT_AST", 1, true) ~= nil, debug_completion_output)
  helpers.assert_true(
    debug_completion_output:find("used compilation unit/project: true/true", 1, true) ~= nil,
    debug_completion_output
  )
  helpers.assert_true(
    debug_completion_output:find("fallback annotation context used: false", 1, true) ~= nil,
    debug_completion_output
  )

  if fixture_spec.deep_mixin then
    local definition_result, definition_err = mcdev_command("mcdev.definition", { context = context })
    helpers.assert_nil(definition_err, "mcdev.definition failed: " .. vim.inspect(definition_err))
    local definition_locations = definition_result.result and definition_result.result.locations or {}
    helpers.assert_true(#definition_locations > 0, "mixin mcdev.definition should return a target")
    helpers.assert_eq(definition_locations[1].metadata.kind, "class")
    helpers.assert_true(
      definition_locations[1].metadata.owner:find("SimpleTarget", 1, true) ~= nil,
      "definition should resolve SimpleTarget; location=" .. vim.inspect(definition_locations[1])
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
    local inject_debug = inject_result.result and inject_result.result.debug or {}
    log_step("inject completion debug: " .. vim.inspect(inject_debug))
    helpers.assert_eq(inject_debug.command, "mcdev.completion")
    helpers.assert_eq(inject_debug.zeroItemReason, nil)
    helpers.assert_eq(inject_debug.parseSource, "JDT_AST")
    helpers.assert_eq(inject_debug.usedCompilationUnit, true)
    helpers.assert_eq(inject_debug.usedJavaProject, true)
    helpers.assert_true((inject_debug.bindingResolvedCount or 0) > 0, "inject completion debug should resolve JDT bindings")
    helpers.assert_eq(inject_debug.semanticContextFound, true)
    helpers.assert_eq(inject_debug.fallbackAnnotationContextUsed, false)
    helpers.assert_eq(#(inject_debug.warnings or {}), 0)
    helpers.assert_true(
      vim.tbl_filter(function(item)
        return item.insertText and item.insertText:find("draw%(Ljava/lang/String;FF%)V", 1, false)
      end, inject_items)[1] ~= nil,
      "mixin inject completion should return descriptor-qualified draw overload"
    )

    local source_only_target_java = workspace .. "/src/main/java/com/example/target/SourceOnlyTarget.java"
    local source_only_target_class = workspace .. "/classpath/com/example/target/SourceOnlyTarget.class"
    helpers.assert_true(
      vim.fn.filereadable(source_only_target_java) == 1,
      "SourceOnlyTarget.java must exist in workspace source roots"
    )
    helpers.assert_true(
      vim.fn.filereadable(source_only_target_class) ~= 1,
      "SourceOnlyTarget must not have bytecode on the e2e classpath"
    )

    local source_only_mixin_path = workspace .. "/src/main/java/com/example/mixin/SourceOnlyMixin.java"
    with_buffer(source_only_mixin_path, "java", {
      "package com.example.mixin;",
      "",
      "import org.spongepowered.asm.mixin.Mixin;",
      "",
      "@Mixin(SourceOnlyT",
      "public abstract class SourceOnlyMixin {",
      "}",
    }, function(source_only_bufnr)
      local mixin_class_line = nil
      for index, line in ipairs(vim.api.nvim_buf_get_lines(source_only_bufnr, 0, -1, false)) do
        if line:find("@Mixin%(SourceOnlyT", 1, false) then
          mixin_class_line = index
          break
        end
      end
      helpers.assert_not_nil(mixin_class_line, "@Mixin(SourceOnlyT line not found")
      local mixin_class_context = build_context(source_only_bufnr, { mixin_class_line, 18 })
      local mixin_class_result, mixin_class_err = mcdev_command("mcdev.completion", {
        context = mixin_class_context,
        trigger = { kind = "manual" },
        options = {
          preferredAtTarget = "descriptor",
          mixinClassInsert = "import",
          injectMethodDescriptor = "auto",
        },
      })
      helpers.assert_nil(mixin_class_err, "source-only @Mixin mcdev.completion failed: " .. vim.inspect(mixin_class_err))
      local mixin_class_items = mixin_class_result.result and mixin_class_result.result.items or {}
      local mixin_class_debug = mixin_class_result.result and mixin_class_result.result.debug or {}
      log_step("source-only mixin class completion debug: " .. vim.inspect(mixin_class_debug))
      helpers.assert_not_nil(
        vim.tbl_filter(function(item)
          return item.label == "SourceOnlyTarget"
        end, mixin_class_items)[1],
        "source-only @Mixin class completion should return SourceOnlyTarget"
      )
      helpers.assert_eq(mixin_class_debug.parseSource, "JDT_AST")
      helpers.assert_eq(mixin_class_debug.usedJavaProject, true)
      helpers.assert_eq(mixin_class_debug.usedCompilationUnit, true)
      helpers.assert_eq(mixin_class_debug.fallbackAnnotationContextUsed, false)

      local source_only_inject_source = {
        "package com.example.mixin;",
        "",
        "import com.example.target.SourceOnlyTarget;",
        "import org.spongepowered.asm.mixin.Mixin;",
        "import org.spongepowered.asm.mixin.injection.Inject;",
        "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
        "",
        "@Mixin(SourceOnlyTarget.class)",
        "public abstract class SourceOnlyMixin {",
        "    @Inject(method = \"",
        "    private void mcdev$onPulse(CallbackInfo ci) {}",
        "}",
      }
      vim.api.nvim_buf_set_lines(source_only_bufnr, 0, -1, false, source_only_inject_source)
      local source_only_inject_line = nil
      for index, line in ipairs(source_only_inject_source) do
        if line:find("method = \"", 1, true) then
          source_only_inject_line = index
          break
        end
      end
      helpers.assert_not_nil(source_only_inject_line, "source-only @Inject method open quote line not found")
      local source_only_inject_context = build_context(source_only_bufnr, { source_only_inject_line, 22 })
      local source_only_inject_result, source_only_inject_err = mcdev_command("mcdev.completion", {
        context = source_only_inject_context,
        trigger = { kind = "manual" },
        options = {
          preferredAtTarget = "descriptor",
          mixinClassInsert = "import",
          injectMethodDescriptor = "always",
        },
      })
      helpers.assert_nil(
        source_only_inject_err,
        "source-only inject mcdev.completion failed: " .. vim.inspect(source_only_inject_err)
      )
      local source_only_inject_items = source_only_inject_result.result and source_only_inject_result.result.items or {}
      local source_only_inject_debug = source_only_inject_result.result and source_only_inject_result.result.debug or {}
      log_step("source-only inject completion debug: " .. vim.inspect(source_only_inject_debug))
      helpers.assert_eq(source_only_inject_debug.parseSource, "JDT_AST")
      helpers.assert_eq(source_only_inject_debug.usedJavaProject, true)
      helpers.assert_eq(source_only_inject_debug.usedCompilationUnit, true)
      helpers.assert_eq(source_only_inject_debug.fallbackAnnotationContextUsed, false)
      helpers.assert_true(
        vim.tbl_filter(function(item)
          return item.insertText == "pulse()V"
        end, source_only_inject_items)[1] ~= nil,
        "source-only inject completion should return pulse()V"
      )
      helpers.assert_true(
        vim.tbl_filter(function(item)
          return item.insertText == "measure(Ljava/lang/String;)I"
        end, source_only_inject_items)[1] ~= nil,
        "source-only inject completion should return measure(Ljava/lang/String;)I"
      )
    end)

    local invoker_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.gen.Invoker;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class ExampleMixin {",
      "    @Invoker(\"\")",
      "    public abstract void invokeDraw(String text, float x, float y);",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, invoker_source)
    local invoker_context = build_context(bufnr, { 9, 14 })
    local invoker_result, invoker_err = mcdev_command("mcdev.completion", {
      context = invoker_context,
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(invoker_err, "mixin invoker mcdev.completion failed: " .. vim.inspect(invoker_err))
    local invoker_debug = invoker_result.result and invoker_result.result.debug or {}
    log_step("invoker completion debug: " .. vim.inspect(invoker_debug))
    helpers.assert_eq(invoker_debug.parseSource, "JDT_AST")
    helpers.assert_eq(invoker_debug.usedCompilationUnit, true)
    helpers.assert_eq(invoker_debug.usedJavaProject, true)
    helpers.assert_true((invoker_debug.bindingResolvedCount or 0) > 0, "invoker completion debug should resolve JDT bindings")
    helpers.assert_eq(invoker_debug.semanticContextFound, true)
    helpers.assert_eq(invoker_debug.fallbackAnnotationContextUsed, false)
    helpers.assert_eq(#(invoker_debug.warnings or {}), 0)

    local handler_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "import org.spongepowered.asm.mixin.injection.Inject;",
      "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class HandlerSnippetMixin {",
      "    @Inject(method = \"draw(Ljava/lang/String;FF)V\", at = @At(\"HEAD\"))",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, handler_source)
    vim.snippet.stop()
    local handler_line = #handler_source - 1
    local handler_text = handler_source[handler_line]
    local handler_result
    local cancel_handler = completion.complete(function(value)
      if not value.isProvisional then handler_result = value end
    end, bufnr, {handler_line, #handler_text}, {source = "e2e-handler-snippet", stream = true})
    local handler_done = vim.wait(30000, function() return handler_result ~= nil end, 50)
    if not handler_done then cancel_handler() end
    helpers.assert_true(handler_done, "handler declaration completion did not finalize")
    helpers.assert_nil(completion.last_error, "handler declaration completion returned an error")
    local handler_item = vim.iter(handler_result.items or {}):find(function(item)
      return item.data and item.data.source == "mixin.handler" and item.data.name == "Inject"
    end)
    helpers.assert_not_nil(handler_item,
      "completed @Inject must offer a handler declaration snippet; project_transport=" ..
      vim.inspect(completion.last_project_transport_error))
    if completion.last_project_transport_error ~= nil then
      log_step("handler declaration validated through native helper/JDT fallback; project transport unavailable: " ..
        tostring(completion.last_project_transport_error))
    end
    helpers.assert_eq(handler_item.insertTextFormat, vim.lsp.protocol.InsertTextFormat.Snippet)
    helpers.assert_true(handler_item.textEdit ~= nil and handler_item.textEdit.newText ~= nil,
      "handler declaration must supply a snippet text edit")
    helpers.assert_true(handler_item.textEdit.newText:find("\n    private void ${1}(", 1, true) ~= nil,
      "handler snippet must start a private method with an empty name")
    helpers.assert_true(handler_item.textEdit.newText:find("\n        $0\n        // TODO\n    }", 1, true) ~= nil,
      "handler body stop must precede the default body")

    local handler_edit = vim.deepcopy(handler_item.textEdit)
    local insertion_cursor = {handler_edit.range.start.line + 1, handler_edit.range.start.character}
    local previous_virtualedit = vim.opt_local.virtualedit:get()
    vim.api.nvim_set_current_buf(bufnr)
    vim.opt_local.virtualedit = "onemore"
    vim.api.nvim_win_set_cursor(0, insertion_cursor)
    helpers.assert_true(vim.deep_equal(vim.api.nvim_win_get_cursor(0), insertion_cursor),
      "handler snippet expansion cursor must be at the text edit start")
    local expanded, expand_error = pcall(vim.snippet.expand, handler_item.textEdit.newText)
    helpers.assert_true(expanded, "native handler snippet expansion failed: " .. tostring(expand_error))
    helpers.assert_true(type(vim.snippet.active) == "function" and vim.snippet.active({direction = 1}),
      "handler expansion must activate the empty method-name tabstop")
    local name_cursor = vim.api.nvim_win_get_cursor(0)
    local name_line = vim.api.nvim_buf_get_lines(bufnr, name_cursor[1] - 1, name_cursor[1], false)[1]
    local open_paren = name_line:find("(", 1, true)
    helpers.assert_not_nil(open_paren, "handler declaration must contain a parameter list")
    helpers.assert_eq(name_cursor[2], open_paren - 1,
      "first handler tabstop must select the complete method name")
    local jumped = vim.snippet.jump(1)
    helpers.assert_true(jumped ~= false, "handler snippet must expose a body tabstop")
    local body_cursor = vim.api.nvim_win_get_cursor(0)
    helpers.assert_true(body_cursor[1] > name_cursor[1],
      "Tab from the method name must move into the generated body")
    vim.opt_local.virtualedit = previous_virtualedit
    vim.snippet.stop()

  end

  if fixture == "fabric-mixinextras" then
    local share_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.sugar.Share;",
      "import com.llamalad7.mixinextras.sugar.ref.LocalIntRef;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "import org.spongepowered.asm.mixin.injection.Inject;",
      "import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @Inject(method = \"draw(Ljava/lang/String;FF)V\", at = @At(\"HEAD\"))",
      "    private void mcdev$shareExisting(CallbackInfo ci, @Share(\"speed\") LocalIntRef speed) {}",
      "",
      "    @Inject(method = \"draw(Ljava/lang/String;FF)V\", at = @At(\"HEAD\"))",
      "    private void mcdev$sharePartial(CallbackInfo ci, @Share(\"sp\") LocalIntRef partial) {}",
      "",
      "    @Inject(method = \"draw(Ljava/lang/String;FF)V\", at = @At(\"HEAD\"))",
      "    private void mcdev$sharePartialNamespaced(CallbackInfo ci, @Share(value = \"sp\", namespace = \"shared\") LocalIntRef partial) {}",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, share_source)
    local share_line, share_character
    for index, line in ipairs(share_source) do
      local share_start = line:find('@Share("sp")', 1, true)
      if share_start then
        share_line = index
        share_character = share_start - 1 + #('@Share("') + #("sp")
        break
      end
    end
    helpers.assert_not_nil(share_line, "@Share(\"sp\") line not found")
    local share_result, share_err = mcdev_command("mcdev.completion", {
      context = build_context(bufnr, { share_line, share_character }),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(share_err, "mixinextras share mcdev.completion failed: " .. vim.inspect(share_err))
    helpers.assert_not_nil(share_result, "mixinextras share mcdev.completion returned no payload")
    local share_items = share_result.result and share_result.result.items or {}
    local share_debug = share_result.result and share_result.result.debug or {}
    helpers.assert_true(
      vim.tbl_filter(function(item)
        return item.insertText == "speed"
      end, share_items)[1] ~= nil,
      string.format(
        "mixinextras share completion should return speed (kind=%s, candidates=%s/%s, warnings=%s)",
        tostring(share_debug.completionContextKind),
        tostring(share_debug.candidateCountBeforeFilter),
        tostring(share_debug.candidateCountAfterFilter),
        vim.inspect(share_debug.warnings or {})
      )
    )
    helpers.assert_eq(share_debug.parseSource, "JDT_AST")
    helpers.assert_eq(share_debug.usedCompilationUnit, true)
    helpers.assert_eq(share_debug.usedJavaProject, true)

    local namespaced_share_line, namespaced_share_character
    for index, line in ipairs(share_source) do
      local share_start = line:find('@Share(value = "sp", namespace = "shared")', 1, true)
      if share_start then
        namespaced_share_line = index
        namespaced_share_character = share_start - 1 + #('@Share(value = "') + #("sp")
        break
      end
    end
    helpers.assert_not_nil(namespaced_share_line, "namespaced @Share value line not found")
    local namespaced_share_result, namespaced_share_err = mcdev_command("mcdev.completion", {
      context = build_context(bufnr, { namespaced_share_line, namespaced_share_character }),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(
      namespaced_share_err,
      "namespaced mixinextras share mcdev.completion failed: " .. vim.inspect(namespaced_share_err)
    )
    helpers.assert_not_nil(namespaced_share_result, "namespaced mixinextras share mcdev.completion returned no payload")
    local namespaced_share_items = namespaced_share_result.result and namespaced_share_result.result.items or {}
    helpers.assert_true(
      vim.tbl_filter(function(item)
        return item.insertText == "speedAcrossFiles"
      end, namespaced_share_items)[1] ~= nil,
      "namespaced mixinextras share completion should return speedAcrossFiles"
    )

    local method_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @ModifyExpressionValue(method = \"dra\", at = @At(value = \"CONSTANT\", args = \"floatValue=0.0\"))",
      "    private float mcdev$modifyX(float original) { return original; }",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, method_source)
    local method_line, method_character
    for index, line in ipairs(method_source) do
      local method_start = line:find('method = "dra"', 1, true)
      if method_start then
        method_line = index
        method_character = method_start - 1 + #('method = "') + #("dra")
        break
      end
    end
    helpers.assert_not_nil(method_line, "mixinextras method completion line not found")
    local method_result, method_err = mcdev_command("mcdev.completion", {
      context = build_context(bufnr, { method_line, method_character }),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(method_err, "mixinextras method completion failed: " .. vim.inspect(method_err))
    local method_items = method_result.result and method_result.result.items or {}
    local method_debug = method_result.result and method_result.result.debug or {}
    helpers.assert_true(
      vim.tbl_filter(function(item)
        return item.insertText and item.insertText:find("draw", 1, true) == 1
      end, method_items)[1] ~= nil,
      "mixinextras method completion should return draw"
    )
    helpers.assert_eq(method_debug.parseSource, "JDT_AST")
    helpers.assert_eq(method_debug.usedCompilationUnit, true)
    helpers.assert_eq(method_debug.usedJavaProject, true)

    local target_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.injector.wrapoperation.Operation;",
      "import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @WrapOperation(method = \"draw(Ljava/lang/String;FF)V\", at = @At(value = \"INVOKE\", target = \"\"))",
      "    private int mcdev$wrap(String instance, Operation<Integer> original) { return original.call(instance); }",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, target_source)
    local target_line, target_character
    for index, line in ipairs(target_source) do
      local target_start = line:find('target = ""', 1, true)
      if target_start then
        target_line = index
        target_character = target_start - 1 + #('target = "')
        break
      end
    end
    helpers.assert_not_nil(target_line, "mixinextras target completion line not found")
    local target_result, target_err = mcdev_command("mcdev.completion", {
      context = build_context(bufnr, { target_line, target_character }),
      trigger = { kind = "manual" },
      options = {
        preferredAtTarget = "descriptor",
        mixinClassInsert = "import",
        injectMethodDescriptor = "auto",
      },
    })
    helpers.assert_nil(target_err, "mixinextras target completion failed: " .. vim.inspect(target_err))
    local target_items = target_result.result and target_result.result.items or {}
    helpers.assert_not_nil(
      vim.tbl_filter(function(item)
        return item.insertText == "Ljava/lang/String;length()I"
      end, target_items)[1],
      "mixinextras target completion should return String.length: " .. vim.inspect({
        debug = target_result.result and target_result.result.debug,
        candidates = vim.tbl_map(function(item) return item.insertText end, target_items),
      })
    )

    local bad_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.injector.wrapoperation.Operation;",
      "import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "import org.spongepowered.asm.mixin.injection.At;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @WrapOperation(method = \"draw(Ljava/lang/String;FF)V\", at = @At(value = \"INVOKE\", target = \"Ljava/lang/String;length()I\"))",
      "    private long mcdev$badReturn(String instance, Operation<Integer> original) { return original.call(instance); }",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, bad_source)
    local bad_line, bad_character
    for index, line in ipairs(bad_source) do
      local bad_start = line:find("mcdev$badReturn", 1, true)
      if bad_start then
        bad_line = index
        bad_character = bad_start - 1 + #("mcdev$")
        break
      end
    end
    helpers.assert_not_nil(bad_line, "mixinextras bad handler line not found")
    local bad_context = build_context(bufnr, { bad_line, bad_character })
    local diagnostics_result, diagnostics_err = mcdev_command("mcdev.diagnostics", { context = bad_context })
    helpers.assert_nil(diagnostics_err, "mixinextras diagnostics failed: " .. vim.inspect(diagnostics_err))
    local diagnostics = diagnostics_result.result and diagnostics_result.result.diagnostics or {}
    local wrong_return = vim.tbl_filter(function(diagnostic)
      return diagnostic.code == "MIXINEXTRAS_WRONG_RETURN_TYPE"
    end, diagnostics)[1]
    helpers.assert_not_nil(wrong_return, "mixinextras diagnostics should report wrong WrapOperation return type")

    local code_action_result, code_action_err = mcdev_command("mcdev.codeAction", {
      context = bad_context,
      range = wrong_return.range,
      diagnosticCodes = { wrong_return.code },
    })
    helpers.assert_nil(code_action_err, "mixinextras code action failed: " .. vim.inspect(code_action_err))
    local actions = code_action_result.result and code_action_result.result.actions or {}
    helpers.assert_not_nil(
      vim.tbl_filter(function(action)
        return action.kind == "quickfix.mixinextras.fixHandlerSignature"
      end, actions)[1],
      "mixinextras code action should offer a handler signature fix"
    )

    local fix_action = vim.tbl_filter(function(action)
      return action.kind == "quickfix.mixinextras.fixHandlerSignature"
    end, actions)[1]
    local before_fix = vim.api.nvim_buf_get_lines(bufnr, bad_line - 1, bad_line, false)[1]
    helpers.assert_true(
      before_fix:find("return original.call(instance);", 1, true) ~= nil,
      "bad WrapOperation handler should start with a compilable body"
    )
    local lsp_action = convert.to_lsp_code_action(fix_action)
    helpers.assert_not_nil(lsp_action.edit, "mixinextras signature fix should include a workspace edit")
    vim.lsp.util.apply_workspace_edit(lsp_action.edit, (client and client.offset_encoding) or "utf-16")

    local after_fix = vim.api.nvim_buf_get_lines(bufnr, bad_line - 1, bad_line, false)[1]
    helpers.assert_true(
      after_fix:find("private int mcdev$badReturn", 1, true) ~= nil,
      "mixinextras signature fix should change the handler return type to int"
    )
    helpers.assert_true(
      after_fix:find("return original.call(instance);", 1, true) ~= nil,
      "mixinextras signature fix should preserve the handler body"
    )
    vim.api.nvim_buf_call(bufnr, function()
      vim.cmd("silent update")
    end)

    local fixed_context = build_context(bufnr, { bad_line, 0 })
    local fixed_diagnostics_result, fixed_diagnostics_err = mcdev_command("mcdev.diagnostics", {
      context = fixed_context,
    })
    helpers.assert_nil(
      fixed_diagnostics_err,
      "mixinextras diagnostics after signature fix failed: " .. vim.inspect(fixed_diagnostics_err)
    )
    local fixed_diagnostics = fixed_diagnostics_result.result and fixed_diagnostics_result.result.diagnostics or {}
    helpers.assert_nil(
      vim.tbl_filter(function(diagnostic)
        return diagnostic.code == "MIXINEXTRAS_WRONG_RETURN_TYPE"
      end, fixed_diagnostics)[1],
      "mixinextras signature fix should remove the wrong return type diagnostic"
    )

    compile_single_java_source(mixin_file)

    local existing_wrap_method_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;",
      "import com.llamalad7.mixinextras.injector.wrapoperation.Operation;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @WrapMethod(method = \"draw(Ljava/lang/String;FF)V\")",
      "    // keep the existing handler below the annotation",
      "    /* and keep this block comment too */",
      "    private void mcdev$existingWrap(String arg0, float arg1, float arg2, Operation<Void> original) {",
      "        original.call(arg0, arg1, arg2);",
      "    }",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, existing_wrap_method_source)
    local existing_wrap_method_line = 10
    local existing_wrap_method_context = build_context(bufnr, { existing_wrap_method_line, 4 })
    local existing_wrap_method_before = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
    local existing_actions_result, existing_actions_err = mcdev_command("mcdev.codeAction", {
      context = existing_wrap_method_context,
      range = {
        start = { line = existing_wrap_method_line - 1, character = 4 },
        ["end"] = { line = existing_wrap_method_line - 1, character = 4 },
      },
      diagnosticCodes = {},
    })
    helpers.assert_nil(
      existing_actions_err,
      "mixinextras existing WrapMethod code action failed: " .. vim.inspect(existing_actions_err)
    )
    local existing_actions = existing_actions_result.result and existing_actions_result.result.actions or {}
    helpers.assert_nil(
      vim.tbl_filter(function(action)
        return action.kind == "quickfix.mixinextras.generateHandler"
      end, existing_actions)[1],
      "mixinextras code action should not generate a second handler after comments"
    )
    helpers.assert_eq(
      table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"),
      existing_wrap_method_before,
      "mixinextras existing WrapMethod code action should preserve the handler body"
    )
    vim.api.nvim_buf_call(bufnr, function()
      vim.cmd("silent update")
    end)
    compile_single_java_source(mixin_file)

    local wrap_method_source = {
      "package com.example.mixin;",
      "",
      "import com.example.target.SimpleTarget;",
      "import com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod;",
      "import org.spongepowered.asm.mixin.Mixin;",
      "",
      "@Mixin(SimpleTarget.class)",
      "public abstract class MixinExtrasExample {",
      "    @WrapMethod(method = \"draw(Ljava/lang/String;FF)V\")",
      "}",
    }
    vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, wrap_method_source)
    local wrap_method_line = 9
    local wrap_method_character = 4
    local wrap_method_context = build_context(bufnr, { wrap_method_line, wrap_method_character })
    local generated_actions_result, generated_actions_err = mcdev_command("mcdev.codeAction", {
      context = wrap_method_context,
      range = {
        start = { line = wrap_method_line - 1, character = wrap_method_character },
        ["end"] = { line = wrap_method_line - 1, character = wrap_method_character },
      },
      diagnosticCodes = { "JAVA_UNRELATED_DIAGNOSTIC" },
    })
    helpers.assert_nil(
      generated_actions_err,
      "mixinextras WrapMethod generation code action failed: " .. vim.inspect(generated_actions_err)
    )
    local generated_actions = generated_actions_result.result and generated_actions_result.result.actions or {}
    local generated_action = vim.tbl_filter(function(action)
      return action.kind == "quickfix.mixinextras.generateHandler"
        and action.title == "Generate WrapMethod handler"
    end, generated_actions)[1]
    helpers.assert_not_nil(
      generated_action,
      "mixinextras code action should generate a WrapMethod handler for a handler-less annotation"
    )
    local generated_lsp_action = convert.to_lsp_code_action(generated_action)
    helpers.assert_not_nil(
      generated_lsp_action.edit,
      "mixinextras WrapMethod generation action should include a workspace edit"
    )
    vim.lsp.util.apply_workspace_edit(generated_lsp_action.edit, (client and client.offset_encoding) or "utf-16")

    local generated_source = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
    helpers.assert_true(
      generated_source:find("Operation<Void>", 1, true) ~= nil
        or generated_source:find("Operation<java.lang.Void>", 1, true) ~= nil,
      "generated WrapMethod handler should declare Operation<Void>"
    )
    helpers.assert_true(
      generated_source:find("original.call(arg0, arg1, arg2);", 1, true) ~= nil,
      "generated WrapMethod handler should call the original with target arguments"
    )
    vim.api.nvim_buf_call(bufnr, function()
      vim.cmd("silent update")
    end)
    compile_single_java_source(mixin_file)

    local generated_handler_cases = {
      {
        name = "ModifyReceiver",
        annotation_import = "import com.llamalad7.mixinextras.injector.ModifyReceiver;",
        annotation = '@ModifyReceiver(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))',
        expected = {
          "String mcdevHandler(String instance)",
          "return instance;",
        },
        forbidden = { "Operation<", "Object mcdevHandler" },
      },
      {
        name = "WrapWithCondition",
        annotation_import = "import com.llamalad7.mixinextras.injector.v2.WrapWithCondition;",
        annotation = '@WrapWithCondition(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Lcom/example/target/SimpleTarget;touch()V"))',
        expected = {
          "boolean mcdevHandler(SimpleTarget simpleTarget)",
          "return true;",
        },
        forbidden = { "Operation<", "Object mcdevHandler" },
      },
      {
        name = "ModifyReturnValue",
        annotation_import = "import com.llamalad7.mixinextras.injector.ModifyReturnValue;",
        annotation = '@ModifyReturnValue(method = "compute()I", at = @At("RETURN"))',
        expected = {
          "int mcdevHandler(int original)",
          "return original;",
        },
        forbidden = { "Operation<", "Object mcdevHandler" },
      },
      {
        name = "ModifyExpressionValue",
        annotation_import = "import com.llamalad7.mixinextras.injector.ModifyExpressionValue;",
        annotation = '@ModifyExpressionValue(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))',
        expected = {
          "int mcdevHandler(int original)",
          "return original;",
        },
        forbidden = { "Operation<", "Object mcdevHandler" },
      },
      {
        name = "WrapOperation",
        annotation_import = "import com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation;",
        annotation = '@WrapOperation(method = "draw(Ljava/lang/String;FF)V", at = @At(value = "INVOKE", target = "Ljava/lang/String;length()I"))',
        expected = {
          "String instance, Operation<Integer> original",
          "return original.call(instance);",
          "import com.llamalad7.mixinextras.injector.wrapoperation.Operation;",
        },
        forbidden = { "Object mcdevHandler" },
      },
    }

    local function generated_handler_source(test_case)
      return {
        "package com.example.mixin;",
        "",
        "import com.example.target.SimpleTarget;",
        test_case.annotation_import,
        "import org.spongepowered.asm.mixin.Mixin;",
        "import org.spongepowered.asm.mixin.injection.At;",
        "",
        "@Mixin(SimpleTarget.class)",
        "public abstract class MixinExtrasExample {",
        "    " .. test_case.annotation,
        "}",
      }
    end

    for _, test_case in ipairs(generated_handler_cases) do
      log_step("mixinextras generation/apply/compile " .. test_case.name)
      local generated_case_source = generated_handler_source(test_case)
      vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, generated_case_source)
      local generated_case_line, generated_case_character = find_line_and_character(
        generated_case_source,
        "@" .. test_case.name
      )
      helpers.assert_not_nil(generated_case_line, test_case.name .. " annotation line not found")
      local generated_case_context = build_context(bufnr, { generated_case_line, generated_case_character })
      local generated_case_result, generated_case_err = mcdev_command("mcdev.codeAction", {
        context = generated_case_context,
        range = {
          start = { line = generated_case_line - 1, character = generated_case_character },
          ["end"] = { line = generated_case_line - 1, character = generated_case_character },
        },
        diagnosticCodes = { "JAVA_UNRELATED_DIAGNOSTIC" },
      })
      helpers.assert_nil(
        generated_case_err,
        "mixinextras " .. test_case.name .. " generation code action failed: " .. vim.inspect(generated_case_err)
      )
      local generated_case_actions = generated_case_result.result and generated_case_result.result.actions or {}
      local generated_case_generation_actions = vim.tbl_filter(function(action)
        return action.kind == "quickfix.mixinextras.generateHandler"
          and action.title == "Generate " .. test_case.name .. " handler"
      end, generated_case_actions)
      helpers.assert_eq(
        #generated_case_generation_actions,
        1,
        "mixinextras " .. test_case.name .. " should offer exactly one generation action; response="
          .. vim.inspect(generated_case_result)
      )

      local generated_case_lsp_action = convert.to_lsp_code_action(generated_case_generation_actions[1])
      helpers.assert_not_nil(
        generated_case_lsp_action.edit,
        "mixinextras " .. test_case.name .. " generation action should include a workspace edit"
      )
      vim.lsp.util.apply_workspace_edit(
        generated_case_lsp_action.edit,
        (client and client.offset_encoding) or "utf-16"
      )

      local generated_case_text = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
      for _, expected_text in ipairs(test_case.expected) do
        helpers.assert_true(
          generated_case_text:find(expected_text, 1, true) ~= nil,
          "mixinextras " .. test_case.name .. " generation should contain " .. expected_text
        )
      end
      for _, forbidden_text in ipairs(test_case.forbidden or {}) do
        helpers.assert_nil(
          generated_case_text:find(forbidden_text, 1, true),
          "mixinextras " .. test_case.name .. " generation should not contain " .. forbidden_text
        )
      end
      helpers.assert_true(
        generated_case_text:find("@" .. test_case.name, 1, true) ~= nil,
        "mixinextras " .. test_case.name .. " generation should preserve its annotation"
      )
      vim.api.nvim_buf_call(bufnr, function()
        vim.cmd("silent update")
      end)
      compile_single_java_source(mixin_file)
    end
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
  local diagnostics_result, diagnostics_err = mcdev_command("mcdev.diagnostics", { context = diagnostics_context })
  helpers.assert_nil(diagnostics_err, "AW mcdev.diagnostics failed: " .. vim.inspect(diagnostics_err))
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
  local at_diagnostics_result, at_diagnostics_err = mcdev_command("mcdev.diagnostics", {
    context = missing_descriptor_context,
  })
  helpers.assert_nil(at_diagnostics_err, "AT mcdev.diagnostics failed: " .. vim.inspect(at_diagnostics_err))
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

if at_file then
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
end

if fixture == "fabric-mixinextras" then
  local workflow_helpers = {
    helpers = helpers,
    with_buffer = with_buffer,
    mixin_file = mixin_file,
    build_context = build_context,
    mcdev_command = mcdev_command,
    compile_single_java_source = compile_single_java_source,
    log_step = log_step,
  }
  dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/expression_e2e.lua")(workflow_helpers)
  dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/sugar_e2e.lua")(workflow_helpers)
end

print("mcdev osgi bundle e2e passed for " .. fixture)
log_step("passed")
