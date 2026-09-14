-- Exercise the failure path, including the actual embedded PowerShell program.
-- Parsing the outer Lua file alone cannot detect a malformed generated command.
local original_dofile, original_wait = dofile, vim.wait
local original_clients, original_cmd = vim.lsp.get_clients, vim.cmd
local original_system, original_stderr = vim.system, io.stderr
local stops, commands, errors, cleanup = {}, {}, {}, nil
local client = {
  stop = function(_, force) stops[#stops + 1] = force end,
  is_stopped = function() return true end,
}
_G.mcdev_e2e_jdtls_exit = { exited = false }
_G.dofile = function() end -- The editor assertions have already finished.
vim.wait = function() return false end -- Process remains alive past the deadline.
vim.lsp.get_clients = function() return { client } end
vim.cmd = function(command) commands[#commands + 1] = command end
io.stderr = { write = function(_, message) errors[#errors + 1] = message end }
vim.system = function(command, options, on_exit)
  vim.wait = original_wait -- The real process waiter must pump the event loop.
  return original_system(command, options, function(result)
    cleanup = result
    if on_exit then on_exit(result) end
  end)
end
local ok, err = xpcall(function()
  assert(loadfile("mcdev-nvim/tests/e2e/run_bundle_e2e.lua"))()
end, debug.traceback)
_G.dofile, vim.wait = original_dofile, original_wait
vim.lsp.get_clients, vim.cmd = original_clients, original_cmd
vim.system, io.stderr = original_system, original_stderr
assert(ok, err)
assert(stops[1] == false and stops[2] == true, "deadline must escalate graceful stop to force")
assert(commands[1] == "cquit 1", "shutdown timeout must fail the test")
assert(table.concat(errors):find("graceful shutdown exceeded", 1, true), "timeout must be reported")
if vim.fn.has("win32") == 1 then
  assert(cleanup and cleanup.code == 0, "generated cleanup program failed: " .. vim.inspect(cleanup))
end
print("E2E teardown timeout path passed")
