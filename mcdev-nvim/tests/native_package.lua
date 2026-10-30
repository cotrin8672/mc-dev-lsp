local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local manifest = vim.json.decode(table.concat(vim.fn.readfile(assert(vim.env.MCDEV_NATIVE_MANIFEST)), "\n"))
local binary = assert(vim.env.MCDEV_COMPANION_BINARY)
h.assert_true(vim.deep_equal(manifest.capabilities, { "semantic-v1" }), "required base stays compatible")
h.assert_eq(#manifest.optionalCapabilities, 6)
h.assert_eq(vim.tbl_count(manifest.supportedOps), 5)

local function negotiate(params)
  local stdout, stderr = {}, {}
  local job = vim.fn.jobstart({ binary }, {
    stdout_buffered = true,
    stderr_buffered = true,
    on_stdout = function(_, data) stdout = data end,
    on_stderr = function(_, data) stderr = data end,
  })
  h.assert_true(job > 0, "start packaged companion")
  local body = vim.json.encode({ jsonrpc = "2.0", id = "package", method = "initialize", params = params })
  vim.fn.chansend(job, "Content-Length: " .. #body .. "\r\n\r\n" .. body)
  vim.fn.chanclose(job, "stdin")
  local code = vim.fn.jobwait({ job }, 5000)[1]
  if code == -1 then
    vim.fn.jobstop(job)
    vim.fn.jobwait({ job }, 1000)
  end
  h.assert_eq(code, 0, "initialization exits within five seconds")
  h.assert_eq(table.concat(stderr, "\n"), "", "initialization has no process errors")
  local length, response = table.concat(stdout, "\n"):match("^Content%-Length: (%d+)\r\n\r\n(.*)$")
  h.assert_not_nil(length, "one framed initialization reply")
  h.assert_eq(#response, tonumber(length), "reply uses byte-counted framing")
  local reply = vim.json.decode(response)
  h.assert_nil(reply.error)
  h.assert_eq(reply.id, "package")
  return reply.result
end

local params = { protocol = { major = 1, minor = 0 }, provider = "mcdev", capabilities = manifest.capabilities }
local base = negotiate(params)
h.assert_true(vim.deep_equal(base.capabilities, manifest.capabilities), "semantic-only host remains supported")
h.assert_nil(base.supportedOps)
h.assert_nil(base.commands)

local expected = vim.list_extend(vim.deepcopy(manifest.capabilities), manifest.optionalCapabilities)
params.capabilities = vim.list_extend(vim.deepcopy(expected), { "future-capability-v1" })
params.supportedOps = vim.deepcopy(manifest.supportedOps)
for _, operations in pairs(params.supportedOps) do
  operations[#operations + 1] = "futureOperation"
end
params.supportedOps["future-capability-v1"] = { "futureOperation" }
params.commands = { "unknown.command" }
local optional = negotiate(params)
h.assert_true(vim.deep_equal(optional.capabilities, expected), "manifest lists exactly the negotiated capabilities")
h.assert_true(vim.deep_equal(optional.supportedOps, manifest.supportedOps), "manifest operations match the binary")
h.assert_eq(#optional.commands, 0, "provider commands still require a configured implemented command")
print("native package metadata: semantic-only fallback, six optional capabilities, exact operations and unknown rejection passed")
