if vim.fn.has("win32") ~= 1 then
  print("Windows JDT process-tree regression is not applicable")
  return
end
local java = (vim.env.JAVA_HOME or error("JAVA_HOME is required")) .. "/bin/java.exe"
local source = vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/TeardownProcess.java"

local function run_snapshot_failure_case(failure_kind)
  local script_path = vim.fn.tempname() .. ".lua"
  local marker_path = vim.fn.tempname()
  local report_path = vim.fn.tempname()
  local taskkill_path = vim.fn.tempname()
  local ready_path = vim.fn.tempname()
  local server_path = vim.fn.tempname()
  local child_source = string.format([=[
vim.opt.runtimepath:prepend(%q)
local ready = %q
local server = vim.system({ %q, %q, %q, ready }, { stdin = true })
vim.fn.writefile({ tostring(server.pid) }, %q)
assert(vim.wait(10000, function() return vim.fn.filereadable(ready) == 1 end, 25), "owned Java child failed to start")
local marker = %q
local report = %q
local taskkill = %q
local original_system = vim.system
dofile = function() end
io.stderr = {
  write = function(_, message)
    vim.fn.writefile({ tostring(message) }, report, "a")
  end,
}
vim.lsp.get_clients = function()
  return {{
    stop = function()
      vim.fn.writefile({ "client-stop-called" }, marker)
    end,
    is_stopped = function() return true end,
  }}
end
vim.system = function(command, options, on_exit)
  if command[1] == "powershell" then
    if %q == "spawn" then error("forced snapshot startup failure") end
    if on_exit then
      vim.schedule(function()
        on_exit({ code = 1, stdout = "", stderr = "forced snapshot failure" })
      end)
    end
    return { kill = function() end }
  end
  if command[1] == "taskkill.exe" then
    vim.fn.writefile({ table.concat(command, "|") }, taskkill)
  end
  return original_system(command, options, on_exit)
end
assert(loadfile(%q))()
]=],
    vim.fn.getcwd() .. "/mcdev-nvim",
    ready_path,
    java, source, source,
    server_path,
    marker_path,
    report_path,
    taskkill_path,
    failure_kind,
    vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/run_bundle_e2e.lua")
  assert(vim.fn.writefile(vim.split(child_source, "\n", { plain = true }), script_path) == 0)

  local child = vim.system({ vim.v.progpath, "--headless", "-u", "NONE", "-l", script_path }, { text = true })
  local result = child:wait(15000)
  if result == nil then
    child:kill(9)
    result = child:wait(2000)
  end

  local report = vim.fn.filereadable(report_path) == 1 and table.concat(vim.fn.readfile(report_path), "\n") or ""
  local taskkill = vim.fn.filereadable(taskkill_path) == 1
      and table.concat(vim.fn.readfile(taskkill_path), "\n") or ""
  local clientStopped = vim.fn.filereadable(marker_path) == 1
  local server_pid = vim.fn.filereadable(server_path) == 1 and tonumber(vim.fn.readfile(server_path)[1])
  local grandchild_pid = vim.fn.filereadable(ready_path) == 1 and tonumber(vim.fn.readfile(ready_path)[1])
  local owned_reaped = server_pid and grandchild_pid and vim.uv.kill(server_pid, 0) == nil
    and vim.uv.kill(grandchild_pid, 0) == nil
  -- These PIDs belong to the newly launched fixture, even if an assertion fails.
  if grandchild_pid then vim.uv.kill(grandchild_pid, 9) end
  if server_pid then vim.uv.kill(server_pid, 9) end
  vim.fn.delete(script_path)
  vim.fn.delete(marker_path)
  vim.fn.delete(report_path)
  vim.fn.delete(taskkill_path)
  vim.fn.delete(ready_path)
  vim.fn.delete(server_path)

  assert(result, "snapshot-failure child did not terminate")
  assert((result.code or 0) ~= 0 or (result.signal or 0) ~= 0, "snapshot failure must fail the child")
  assert(report:find("E2E teardown aborted: JDT process snapshot failed", 1, true),
    "snapshot failure must be reported: " .. report)
  assert(taskkill == "taskkill.exe|/PID|" .. child.pid .. "|/T|/F",
    "snapshot failure must use native current-tree taskkill: " .. taskkill)
  assert(owned_reaped, "snapshot failure fallback must reap the owned Java parent and child")
  assert(not clientStopped, "client.stop must not run without an ownership snapshot")
end

local function run_case(graceful)
  local ready_file = vim.fn.tempname()
  local jdtls_exit = { exited = false }
  local unrelated_exited = false
  local unrelated = vim.system({ java, source }, {}, function() unrelated_exited = true end)
  local server = vim.system({ java, "-Declipse.application=org.eclipse.jdt.ls.core.id1", source, source, ready_file },
    { stdin = true }, function(result)
      jdtls_exit.exited, jdtls_exit.code, jdtls_exit.signal = true, result.code, result.signal
    end)
  local original_dofile, original_clients = dofile, vim.lsp.get_clients
  local original_cmd, original_stderr = vim.cmd, io.stderr
  local commands, errors, stopping = {}, {}, false
  local child_pid
  local ok, err = xpcall(function()
    assert(vim.wait(10000, function() return vim.uv.fs_stat(ready_file) ~= nil end, 25), "test server failed to start")
    child_pid = tonumber(vim.fn.readfile(ready_file)[1])
    assert(child_pid, "server must record its child's PID")
    _G.mcdev_e2e_jdtls_exit = jdtls_exit
    _G.dofile = function() end
    vim.lsp.get_clients = function() return { {
      stop = function(_, force)
        stopping = true
        if force then
          server:kill(9)
        elseif graceful then
          -- Like the LSP shutdown response, the exit notification is scheduled
          -- on Neovim's normal event loop, not delivered inside a fast callback.
          vim.schedule(function() server:write("exit\n") end)
        end
      end,
      is_stopped = function() return stopping end,
    } } end
    vim.cmd = function(command) commands[#commands + 1] = command end
    io.stderr = { write = function(_, message) errors[#errors + 1] = message end }
    assert(loadfile("mcdev-nvim/tests/e2e/run_bundle_e2e.lua"))()
    assert(vim.wait(2000, function() return jdtls_exit.exited end, 25), "server transport must settle after cleanup")
    if graceful then
      assert(commands[1] == "qa!", "graceful server with inherited child pipes should finish: " .. table.concat(errors))
      assert(jdtls_exit.code == 0 and jdtls_exit.signal == 0, "server must exit normally")
    else
      assert(commands[1] == "cquit 1", "unresponsive server must fail the test")
      assert(table.concat(errors):find("graceful shutdown exceeded", 1, true), "shutdown timeout must be reported")
    end
    assert(vim.uv.kill(server.pid, 0) == nil, "owned server must be reaped")
    assert(vim.uv.kill(child_pid, 0) == nil, "owned child must be reaped")
    assert(not unrelated_exited and vim.uv.kill(unrelated.pid, 0) ~= nil, "unrelated Java process must survive cleanup")
  end, debug.traceback)
  _G.dofile, vim.lsp.get_clients = original_dofile, original_clients
  vim.cmd, io.stderr = original_cmd, original_stderr
  _G.mcdev_e2e_jdtls_exit = nil
  -- Always stop only this test's processes, including when the assertion fails.
  if child_pid then vim.uv.kill(child_pid, 9) end
  if not jdtls_exit.exited then server:kill(9) end
  unrelated:kill(9)
  vim.wait(5000, function() return jdtls_exit.exited and unrelated_exited end, 25)
  vim.fn.delete(ready_file)
  assert(ok, err)
end
run_case(true)
run_case(false)
run_snapshot_failure_case("exit")
run_snapshot_failure_case("spawn")
print("E2E normal and hung process teardown passed; inherited children reaped and unrelated Java preserved")
