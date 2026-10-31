-- Unit checks for restored E2E control flow; these responses are mocks, not real-host evidence.
local helpers = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local restored = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/kotlin_drill_e2e.lua")
local dependency_check = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/fixed_compile_dependencies.lua")
local workspace = vim.fn.getcwd() .. "/build/nvim-test-logs/kotlin-drill-restore-unit-" .. vim.fn.getpid()
local workspace_uri = vim.uri_from_fname(workspace)
local drill_descriptor = "(Lnet/minecraft/world/item/ItemStack;Lnet/minecraft/world/level/block/state/BlockState;"
  .. "Lnet/minecraft/world/level/Level;Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/entity/player/Player;"
  .. "Lnet/minecraft/world/InteractionHand;Lnet/minecraft/world/phys/BlockHitResult;)Lnet/minecraft/world/ItemInteractionResult;"
local block_target = "Lnet/minecraft/world/level/block/Block;setPlacedBy(Lnet/minecraft/world/level/Level;"
  .. "Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;"
  .. "Lnet/minecraft/world/entity/LivingEntity;Lnet/minecraft/world/item/ItemStack;)V"
local saved_protocol = package.loaded["mcdev.protocol"]
local function kotlin_case(failure)
  local bufnr, requests = nil, 0
  local protocol = {}
  protocol.active_ktlsp_client = function() return { id = 42 } end
  protocol.context = function(buf, position)
    return { documentUri = vim.uri_from_bufnr(buf), languageId = vim.bo[buf].filetype,
      workspaceRoot = workspace_uri, bufferText = table.concat(vim.api.nvim_buf_get_lines(buf, 0, -1, false), "\n"),
      position = { line = position[1] - 1, character = position[2] } }
  end
  protocol.build_completion_payload = function(buf, position)
    return { context = protocol.context(buf, position), options = { injectMethodDescriptor = "auto" } }
  end
  package.loaded["mcdev.protocol"] = protocol
  local ok, err = pcall(restored, {
    helpers = helpers, workspace = workspace, workspace_root_uri = workspace_uri, client_id = 42,
    log_step = function() end,
    with_buffer = function(file, filetype, _, callback)
      bufnr = vim.api.nvim_create_buf(true, false)
      vim.api.nvim_buf_set_name(bufnr, file)
      vim.api.nvim_set_current_buf(bufnr)
      vim.bo[bufnr].filetype = filetype
      callback(bufnr)
    end,
    find_line_and_character = function(lines, marker)
      for line, text in ipairs(lines) do
        local at = text:find(marker, 1, true)
        if at then return line, at - 1 end
      end
    end,
    mcdev_command = function(command, payload, timeout)
      requests = requests + 1
      helpers.assert_eq(timeout, 60000)
      local text = payload.context.bufferText
      local result
      if command == "mcdev.completion" then
        local partial, expected, metadata
        if text:find('"useIt"', 1, true) then
          helpers.assert_eq(payload.options.injectMethodDescriptor, "always")
          partial, expected = "useIt", "useItemOn" .. drill_descriptor
          metadata = { owner = "com/simibubi/create/content/kinetics/drill/DrillBlock", descriptor = drill_descriptor }
        elseif text:find('"HE"', 1, true) then
          partial, expected, metadata = "HE", "HEAD", {}
        elseif text:find('"placeInW"', 1, true) then
          helpers.assert_eq(payload.options.injectMethodDescriptor, "never")
          partial, expected, metadata = "placeInW", "placeInWorld", {}
        else
          partial, expected, metadata = "Lnet/minecraft/world/level/block/Block;setPl", block_target, { name = "setPlacedBy" }
        end
        local position = payload.context.position
        local item = { edit = { newText = expected, range = {
          start = { line = position.line, character = position.character - #partial },
          ["end"] = position } }, metadata = metadata }
        local parse_source = partial == "HE" and "HAND_WRITTEN_FALLBACK" or "HOST_NORMALIZED"
        if failure == "indexed-label" and partial ~= "HE" then parse_source = "HAND_WRITTEN_FALLBACK" end
        if failure == "noindex-label" and partial == "HE" then parse_source = "HOST_NORMALIZED" end
        result = { debug = { parseSource = parse_source,
          usedCompilationUnit = false }, items = failure == "completion" and {} or { item } }
      elseif command == "mcdev.diagnostics" then
        result = { diagnostics = text:find("doesNotExist", 1, true)
          and { { severity = "error", code = "MIXIN_UNRESOLVED_INJECT_METHOD" } }
          or (failure == "placement-diagnostics" and text:find("KotlinPlacementMixin", 1, true)
            and { { severity = "error", code = "UNEXPECTED" } } or {}) }
      elseif command == "mcdev.hover" then
        result = { contents = { "useItemOn" .. drill_descriptor } }
      elseif command == "mcdev.definition" then
        result = { locations = failure == "definition" and {} or { { metadata = { name = "useItemOn",
          descriptor = drill_descriptor }, resolution = "source", documentUri = workspace_uri .. "/DrillBlock.java" } } }
      else
        error("unexpected Kotlin command " .. command)
      end
      return { result = result }
    end,
  })
  package.loaded["mcdev.protocol"] = saved_protocol
  if bufnr then vim.api.nvim_buf_delete(bufnr, { force = true }) end
  if not failure then
    helpers.assert_true(ok, tostring(err))
    helpers.assert_eq(requests, 9, "both Kotlin scenarios must execute every original command")
  else
    helpers.assert_true(not ok, "restored assertion must reject " .. failure)
    local message = ({ completion = "Kotlin completion missing", definition = "Kotlin method definition missing",
      ["placement-diagnostics"] = "valid Kotlin MC diagnostics", ["indexed-label"] = "HOST_NORMALIZED",
      ["noindex-label"] = "HAND_WRITTEN_FALLBACK" })[failure]
    helpers.assert_true(tostring(err):find(message, 1, true) ~= nil, tostring(err))
  end
end
kotlin_case()
for _, failure in ipairs({ "completion", "definition", "placement-diagnostics", "indexed-label", "noindex-label" }) do kotlin_case(failure) end

local function write_bytes(file, bytes)
  local fd = assert(vim.uv.fs_open(file, "w", 438))
  assert(vim.uv.fs_write(fd, bytes, 0))
  assert(vim.uv.fs_close(fd))
end
vim.fn.mkdir(workspace .. "/libs", "p")
local function read_bytes(file)
  local fd = assert(vim.uv.fs_open(file, "r", 438))
  local stat = assert(vim.uv.fs_fstat(fd))
  local bytes = assert(vim.uv.fs_read(fd, stat.size, 0))
  assert(vim.uv.fs_close(fd))
  return bytes
end
local receipt_path = assert(vim.env.MCDEV_E2E_INPUTS_RECEIPT,
  "this selfcheck requires a real fixed-input receipt through MCDEV_E2E_INPUTS_RECEIPT")
local receipt = read_bytes(receipt_path)
local manifest = vim.json.decode(receipt)
local copies = {}
for _, name in ipairs({ "mixin-0.8.7.jar", "mixinextras-fabric-0.5.5.jar" }) do
  local pinned = assert(manifest.artifacts[name], "real fixed receipt is missing " .. name)
  local bytes = read_bytes(pinned.path)
  helpers.assert_eq(#bytes, pinned.size, "real fixed input size must match: " .. name)
  helpers.assert_eq(vim.fn.sha256(bytes), pinned.sha256, "real fixed input SHA-256 must match: " .. name)
  local file = workspace .. "/libs/" .. name
  write_bytes(file, bytes)
  copies[name] = { copy = file, sha256 = pinned.sha256, size = pinned.size }
end
write_bytes(workspace .. "/fixed-inputs-receipt.json", receipt)
local dependencies = { receiptSha256 = vim.fn.sha256(receipt), artifacts = copies }
local function write_dependencies()
  write_bytes(workspace .. "/fixed-dependencies.json", vim.json.encode(dependencies))
end
write_dependencies()
write_bytes(workspace .. "/build.gradle", 'dependencies {\ncompileOnly files("libs/mixin-0.8.7.jar")\n'
  .. 'compileOnly files("libs/mixinextras-fabric-0.5.5.jar")\n}\n')
dependency_check({ helpers = helpers, workspace = workspace })
local extras = copies["mixinextras-fabric-0.5.5.jar"]
copies["mixinextras-fabric-0.5.5.jar"] = nil
write_dependencies()
local ok, err = pcall(dependency_check, { helpers = helpers, workspace = workspace })
helpers.assert_true(not ok and tostring(err):find("MixinExtras Fabric 0.5.5 jar is required", 1, true) ~= nil)
copies["mixinextras-fabric-0.5.5.jar"] = extras
write_dependencies()
local wrong_artifact = manifest.artifacts["mixinextras-neoforge-0.5.3.jar"] or manifest.artifacts["mixin-0.8.7.jar"]
write_bytes(extras.copy, read_bytes(wrong_artifact.path))
ok, err = pcall(dependency_check, { helpers = helpers, workspace = workspace })
helpers.assert_true(not ok and tostring(err):find("physical compile JAR", 1, true) ~= nil)

local bundle = table.concat(vim.fn.readfile("mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua"), "\n")
local guard_start = assert(bundle:find("  helpers.assert_not_nil(info_result.result.jarLocation,", 1, true))
local guard_end = assert(bundle:find("\n  helpers.assert_true(", guard_start, true))
local guard = assert(loadstring("return function(helpers, info_result, companion_cmd)\n"
  .. bundle:sub(guard_start, guard_end - 1) .. "\nend"))()
local executable = vim.fn.exepath("nvim")
guard(helpers, { result = { jarLocation = executable } }, executable)
ok = pcall(guard, helpers, { result = { executableLocation = executable } }, executable)
helpers.assert_true(not ok, "executableLocation alone cannot satisfy the jarLocation contract")
ok = pcall(guard, helpers, { result = { jarLocation = workspace .. "/wrong.exe" } }, executable)
helpers.assert_true(not ok, "jarLocation must identify the selected native artifact")
print("restored Kotlin flow: 1 positive + 5 negative mocks; real fixed receipt/JAR prerequisites and exact jarLocation guards passed")
