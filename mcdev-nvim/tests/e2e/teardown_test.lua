-- Client-stop unit checks; teardown_process_test.lua executes the real programs and process trees.
local function run_case(graceful, snapshot_failure)
  local original_dofile, original_wait = dofile, vim.wait
  local original_clients, original_cmd = vim.lsp.get_clients, vim.cmd
  local original_system, original_stderr = vim.system, io.stderr
  local original_exit = rawget(_G, "mcdev_e2e_ktlsp_exit")
  local stops, commands, errors, cleanup = {}, {}, {}, nil
  local shell_calls, emergency = 0, {}
  local snapshot = {
    roots = { { pid = 710001, creationDate = "2026-01-01T00:00:00.0000000Z", name = "ktlsp.exe", depth = 0 } },
    children = { { pid = 710002, creationDate = "2026-01-01T00:00:01.0000000Z", name = "child.exe", depth = 1 } },
  }
  local exit = { exited = false }
  local client = {
    stop = function(_, force)
      stops[#stops + 1] = force
      if graceful then exit.exited, exit.code, exit.signal = true, 0, 0 end
    end,
    is_stopped = function() return #stops > 0 end,
  }
  _G.mcdev_e2e_ktlsp_exit = exit
  _G.dofile = function() end -- The editor assertions have already finished.
  vim.wait = function(_, predicate) return predicate() end
  vim.lsp.get_clients = function() return { client } end
  vim.cmd = function(command) commands[#commands + 1] = command end
  io.stderr = { write = function(_, message) errors[#errors + 1] = message end }
  vim.system = function(command, options, on_exit)
    local result = { code = 0, signal = 0, stdout = "", stderr = "" }
    if command[1] == "powershell" then
      shell_calls = shell_calls + 1
      if shell_calls == 1 then
        if snapshot_failure then
          result.code, result.stderr = 1, "unit snapshot failure"
        else
          result.stdout = vim.json.encode(snapshot)
        end
      else
        assert(vim.deep_equal(vim.json.decode(options.stdin), snapshot), "owned snapshot must reach process waits and cleanup")
        if shell_calls == 2 then
          result.code = graceful and 0 or 2
          result.stdout = vim.json.encode({ rootsExited = graceful, alive = graceful and {} or { 710001 } })
        elseif shell_calls == 3 then
          local force = graceful and "$false" or "$true"
          assert(command[4]:find("if (" .. force .. ")", 1, true), "owned cleanup must use the expected force mode")
          result.stdout = vim.json.encode({ cleaned = true, remaining = {} })
          cleanup = result
        else
          error("unexpected teardown subprocess phase")
        end
      end
    elseif command[1] == "taskkill.exe" then
      emergency[#emergency + 1] = vim.deepcopy(command)
    else
      error("unit teardown must not execute a subprocess: " .. vim.inspect(command))
    end
    if on_exit then on_exit(result) end
    return { wait = function() return result end, kill = function() end }
  end
  local ok, err = xpcall(function()
    assert(loadfile("mcdev-nvim/tests/e2e/run_bundle_e2e.lua"))()
  end, debug.traceback)
  _G.dofile, vim.wait = original_dofile, original_wait
  vim.lsp.get_clients, vim.cmd = original_clients, original_cmd
  vim.system, io.stderr = original_system, original_stderr
  rawset(_G, "mcdev_e2e_ktlsp_exit", original_exit)
  assert(ok, err)
  if snapshot_failure then
    assert(#stops == 0, "client stop must not run without an ownership snapshot")
    assert(commands[1] == "cquit 1", "snapshot failure must fail the unit case")
    assert(table.concat(errors):find("process snapshot failed", 1, true), "snapshot failure must be reported")
    assert(vim.deep_equal(emergency, { { "taskkill.exe", "/PID", tostring(vim.fn.getpid()), "/T", "/F" } }),
      "unit emergency fallback must remain isolated and target only the current tree")
  else
    if graceful then
      assert(#stops == 1 and stops[1] == false, "graceful exit must not force the client")
      assert(commands[1] == "qa!", "graceful shutdown must finish successfully")
      assert(#errors == 0, "graceful shutdown must not report errors")
    else
      assert(stops[1] == false and stops[2] == true, "deadline must escalate graceful stop to force")
      assert(commands[1] == "cquit 1", "shutdown timeout must fail the test")
      assert(table.concat(errors):find("graceful shutdown exceeded", 1, true), "timeout must be reported")
    end
    assert(#emergency == 0, "client shutdown unit cases must not invoke the emergency fallback")
    if vim.fn.has("win32") == 1 then
      assert(shell_calls == 3, "unit shutdown must check snapshot, root exit and owned cleanup")
      assert(cleanup and cleanup.code == 0, "generated cleanup program failed: " .. vim.inspect(cleanup))
    end
  end
end
run_case(true)
run_case(false)
if vim.fn.has("win32") == 1 then run_case(false, true) end
print("E2E teardown timeout path passed")
