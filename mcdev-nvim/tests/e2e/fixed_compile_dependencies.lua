-- Frozen jdtls_bundle_e2e.lua:302-303; require the exact two physical compile inputs.
return function(h)
  local helpers = h.helpers
  local workspace = h.workspace
  local function read_bytes(path)
    local fd = vim.uv.fs_open(path, "r", 438)
    helpers.assert_not_nil(fd, "fixed compile input must be readable: " .. path)
    local stat = vim.uv.fs_fstat(fd)
    local bytes = stat and vim.uv.fs_read(fd, stat.size, 0)
    vim.uv.fs_close(fd)
    helpers.assert_not_nil(bytes, "fixed compile input read failed: " .. path)
    return bytes
  end
  local dependency_path = workspace .. "/fixed-dependencies.json"
  local dependencies = vim.fn.filereadable(dependency_path) == 1
    and vim.json.decode(read_bytes(dependency_path)) or {}
  local artifacts = dependencies.artifacts or {}
  local mixin_jar = artifacts["mixin-0.8.7.jar"] and artifacts["mixin-0.8.7.jar"].copy
  local mixinextras_jar = artifacts["mixinextras-fabric-0.5.5.jar"] and artifacts["mixinextras-fabric-0.5.5.jar"].copy
  helpers.assert_not_nil(mixin_jar, "Mixin 0.8.7 jar is required for javac E2E verification")
  helpers.assert_not_nil(mixinextras_jar, "MixinExtras Fabric 0.5.5 jar is required for javac E2E verification")
  local receipt_text = read_bytes(workspace .. "/fixed-inputs-receipt.json")
  helpers.assert_eq(vim.fn.sha256(receipt_text), dependencies.receiptSha256,
    "compile dependency receipt must match the verified fixed-input copy")
  local receipt = vim.json.decode(receipt_text)
  helpers.assert_eq(#(receipt.failures or {}), 0, "fixed compile receipt must have no failures")
  local build = read_bytes(workspace .. "/build.gradle")
  for _, name in ipairs({ "mixin-0.8.7.jar", "mixinextras-fabric-0.5.5.jar" }) do
    local selected = artifacts[name]
    local pinned = receipt.artifacts and receipt.artifacts[name]
    helpers.assert_not_nil(pinned, "fixed receipt must select " .. name)
    local copy = workspace .. "/libs/" .. name
    helpers.assert_eq(assert(vim.uv.fs_realpath(selected.copy)), assert(vim.uv.fs_realpath(copy)),
      "compile must use the exact workspace copy of " .. name)
    helpers.assert_eq(selected.sha256, pinned.sha256, "selected SHA-256 must match fixed receipt: " .. name)
    helpers.assert_eq(selected.size, pinned.size, "selected size must match fixed receipt: " .. name)
    helpers.assert_eq(pinned.readOnly, true, "fixed dependency must be a verified read-only input: " .. name)
    local bytes = read_bytes(copy)
    helpers.assert_eq(#bytes, pinned.size, "physical compile JAR size mismatch: " .. name)
    helpers.assert_eq(vim.fn.sha256(bytes), pinned.sha256, "physical compile JAR SHA-256 mismatch: " .. name)
    helpers.assert_true(build:find('compileOnly files("libs/' .. name .. '")', 1, true) ~= nil,
      "Gradle compile must select the fixed physical JAR: " .. name)
  end
  return mixin_jar, mixinextras_jar
end
