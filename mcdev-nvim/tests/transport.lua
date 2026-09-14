local root = vim.fn.getcwd()
package.path = root .. "/mcdev-nvim/lua/?.lua;" .. root .. "/mcdev-nvim/lua/?/init.lua;" .. package.path

local helpers = dofile(root .. "/mcdev-nvim/tests/test_helpers.lua")
require("mcdev")
local transport = require("mcdev.transport")

helpers.assert_true(
  type(vim.lsp.handlers[transport.notification_method]) == "function",
  "mcdev must install the transport notification handler when loaded"
)

local original_get_clients = vim.lsp.get_clients
local active_client = { id = 71, name = "jdtls", config = { root_dir = root } }
vim.lsp.handlers[transport.notification_method](nil, {
  protocolVersion = 1,
  host = "127.0.0.1",
  port = 38740,
  token = string.rep("z", 43),
  timeoutMs = 250,
}, { client_id = active_client.id })
helpers.assert_eq(transport.endpoint(active_client.id).port, 38740)

vim.lsp.get_clients = function(opts)
  if opts and (opts.bufnr == 0 or opts.name == "jdtls") then
    return { active_client }
  end
  return {}
end

transport.handle_notification(nil, {
  command = transport.notification_command,
  arguments = {
    {
      protocolVersion = 1,
      host = "127.0.0.1",
      port = 38741,
      token = string.rep("a", 43),
      timeoutMs = 250,
    },
  },
}, { client_id = active_client.id })
helpers.assert_eq(transport.endpoint(active_client.id).port, 38741)

active_client = { id = 74, name = "jdtls", config = { root_dir = root } }
transport.handle_notification(nil, {
  {
    protocolVersion = 1,
    host = "127.0.0.1",
    port = 38743,
    token = string.rep("c", 43),
    timeoutMs = 250,
  },
}, { client_id = active_client.id })
helpers.assert_eq(transport.endpoint(active_client.id).port, 38743)

transport.handle_notification(nil, {
  protocolVersion = 1,
  host = "192.0.2.1",
  port = 38744,
  token = string.rep("d", 43),
}, { client_id = 75 })
helpers.assert_nil(transport.endpoint(75))

active_client = { id = 73, name = "jdtls", config = { root_dir = root } }
local cancelled, unavailable = transport.request({}, function() end, 0)
helpers.assert_nil(cancelled)
helpers.assert_true(unavailable:find("unavailable", 1, true) ~= nil)

transport.handle_notification(nil, { protocolVersion = 2, host = "127.0.0.1", port = 38742, token = string.rep("b", 43) }, {
  client_id = 72,
})
helpers.assert_nil(transport.endpoint(72))
transport.clear(71)
helpers.assert_nil(transport.endpoint(71))

local ready_count = 0
local removed_count = 0
local stop_ready = transport.when_ready(0, function() ready_count = ready_count + 1 end)
local remove_ready = transport.when_ready(0, function() removed_count = removed_count + 1 end)
remove_ready()
local ready_endpoint = { protocolVersion = 1, host = "127.0.0.1", port = 38745, token = string.rep("f", 43) }
transport.handle_notification(nil, ready_endpoint, { client_id = active_client.id })
transport.handle_notification(nil, ready_endpoint, { client_id = active_client.id })
helpers.assert_eq(ready_count, 1, "endpoint readiness must deliver once")
helpers.assert_eq(removed_count, 0, "cancelled request must not resume at readiness")
stop_ready()

local project_ready = 0
transport.when_ready(0, function() project_ready = project_ready + 1 end)
vim.lsp.handlers["language/status"](nil, { type = "Starting" }, { client_id = active_client.id })
helpers.assert_eq(project_ready, 0)
vim.lsp.handlers["language/status"](nil, { type = "Started" }, { client_id = active_client.id })
vim.lsp.handlers["language/status"](nil, { type = "ServiceReady" }, { client_id = active_client.id })
helpers.assert_eq(project_ready, 1, "JDT project readiness must resume a pending completion once")
local generation = transport.ready_generation(0)
vim.lsp.handlers["language/status"](nil, { type = "ServiceReady" }, { client_id = active_client.id })
transport.when_ready(0, function() project_ready = project_ready + 1 end, generation)
helpers.assert_true(vim.wait(2000, function() return project_ready == 2 end),
  "readiness arriving during the request must not be lost before listener registration")
local cancel_raced = transport.when_ready(0, function() project_ready = project_ready + 1 end, generation)
cancel_raced()
vim.wait(20, function() return false end)
helpers.assert_eq(project_ready, 2, "cancelled late readiness must not resume a request")

-- Exercise libuv's real fast-event callback: completion consumers call Neovim APIs.
local server = assert(vim.uv.new_tcp())
assert(server:bind("127.0.0.1", 0))
local peer
assert(server:listen(1, function(err)
  assert(not err, err)
  peer = assert(vim.uv.new_tcp())
  assert(server:accept(peer))
  peer:read_start(function(read_err, data)
    assert(not read_err, read_err)
    if not data then return end
    peer:read_stop()
    local body = '{"result":{"items":[]}}'
    peer:write("HTTP/1.1 200 OK\r\nContent-Length: " .. #body .. "\r\n\r\n" .. body)
  end)
end))
transport.handle_notification(nil, {
  protocolVersion = 1, host = "127.0.0.1", port = server:getsockname().port,
  token = string.rep("e", 43),
}, { client_id = active_client.id })
local received, response_error, fast_event
local cancel = assert(transport.request({}, function(response, err)
  fast_event = vim.in_fast_event()
  response_error = err
  received = response
  helpers.assert_true(vim.api.nvim_buf_is_valid(0))
end, 0))
local completed = vim.wait(2000, function() return received ~= nil or response_error ~= nil end)
cancel()
if peer and not peer:is_closing() then peer:close() end
server:close()
helpers.assert_true(completed, "real socket completion must settle")
helpers.assert_nil(response_error)
helpers.assert_eq(fast_event, false, "completion must run on Neovim's main loop")
vim.lsp.get_clients = original_get_clients
print("mcdev transport lua tests passed")
