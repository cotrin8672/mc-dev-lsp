local ok, err = xpcall(function()
  dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/e2e/jdtls_bundle_e2e.lua")
end, debug.traceback)

local wait = vim.wait
local function append_error(message)
  ok = false
  err = tostring(err or "") .. "\n" .. message
end

local function as_list(value)
  if value == nil then
    return {}
  end
  if type(value) ~= "table" then
    return { value }
  end
  if next(value) == nil then
    return {}
  end
  if value[1] == nil then
    return { value }
  end
  return value
end

local function powershell(script, stdin, timeout_ms)
  local options = { text = true }
  if stdin ~= nil then
    options.stdin = stdin
  end
  local result
  local started, process = pcall(vim.system, { "powershell", "-NoProfile", "-Command", script }, options, function(value)
    result = value
  end)
  if not started then
    return { code = -1, stdout = "", stderr = tostring(process) }
  end
  -- SystemObj:wait processes only fast events. LSP shutdown/exit callbacks need
  -- ordinary scheduled events too, or waiting for Java exit deadlocks the client.
  if not vim.wait(timeout_ms, function() return result ~= nil end, 25) then
    process:kill(9)
    vim.wait(2000, function() return result ~= nil end, 25)
    return nil
  end
  return result
end

local function decode_snapshot(result)
  if result == nil then
    return nil, "JDT process snapshot timed out"
  end
  if result.code ~= 0 then
    return nil, "JDT process snapshot failed: " .. tostring(result.stderr or result.code)
  end
  local decoded_ok, decoded = pcall(vim.json.decode, result.stdout or "")
  if not decoded_ok or type(decoded) ~= "table" then
    return nil, "JDT process snapshot returned invalid JSON"
  end
  local snapshot = {
    roots = as_list(decoded.roots),
    children = as_list(decoded.children),
  }
  for _, process in ipairs(snapshot.roots) do
    if type(process) ~= "table" or type(process.pid) ~= "number" or type(process.creationDate) ~= "string" then
      return nil, "JDT process snapshot returned an invalid root identity"
    end
  end
  for _, process in ipairs(snapshot.children) do
    if type(process) ~= "table" or type(process.pid) ~= "number" or type(process.creationDate) ~= "string" then
      return nil, "JDT process snapshot returned an invalid child identity"
    end
  end
  return snapshot
end

local function terminate_current_process_tree()
  local started, process = pcall(vim.system, {
    "taskkill.exe",
    "/PID",
    tostring(vim.fn.getpid()),
    "/T",
    "/F",
  }, { text = true })
  if not started then
    return false, "native taskkill fallback could not start: " .. tostring(process)
  end
  local waited, result = pcall(function() return process:wait(5000) end)
  if not waited then
    return false, "native taskkill fallback failed while waiting: " .. tostring(result)
  end
  if result == nil then
    pcall(process.kill, process, 9)
    pcall(process.wait, process, 1000)
    return false, "native taskkill fallback timed out"
  end
  if result.code ~= 0 then
    return false, "native taskkill fallback failed: " .. tostring(result.stderr or result.code)
  end
  return true
end

local function abort_without_process_ownership(snapshot_error)
  append_error(snapshot_error)
  io.stderr:write("E2E teardown aborted: " .. tostring(snapshot_error) .. "\n")
  local _, cleanup_error = terminate_current_process_tree()
  if cleanup_error then
    append_error(cleanup_error)
    io.stderr:write("E2E teardown process-tree fallback failed: " .. cleanup_error .. "\n")
  end
  vim.cmd("cquit 1")
end

local snapshot_script = [=[
$ErrorActionPreference = 'Stop'
$root = ]=] .. tostring(vim.fn.getpid()) .. ";\n" .. [=[
$all = @(Get-CimInstance Win32_Process -ErrorAction Stop)
$byPid = @{}
$children = @{}
foreach ($process in $all) {
    $processId = [int]$process.ProcessId
    $byPid[$processId] = $process
    $parent = [int]$process.ParentProcessId
    if (-not $children.ContainsKey($parent)) {
        $children[$parent] = [System.Collections.Generic.List[int]]::new()
    }
    $children[$parent].Add($processId)
}
$descendants = [System.Collections.Generic.HashSet[int]]::new()
$pending = [System.Collections.Generic.Queue[int]]::new()
$pending.Enqueue($root)
while ($pending.Count -gt 0) {
    $parent = $pending.Dequeue()
    if (-not $children.ContainsKey($parent)) { continue }
    foreach ($child in $children[$parent]) {
        if ($descendants.Add($child)) { $pending.Enqueue($child) }
    }
}
function Identity($process, $depth) {
    [pscustomobject]@{
        pid = [int]$process.ProcessId
        creationDate = $process.CreationDate.ToUniversalTime().ToString('o')
        name = [string]$process.Name
        depth = [int]$depth
    }
}
$servers = @($all | Where-Object {
    $descendants.Contains([int]$_.ProcessId) -and
        $_.Name -ieq 'java.exe' -and
        ([string]$_.CommandLine) -like '*-Declipse.application=org.eclipse.jdt.ls.core.id1*'
})
$roots = [System.Collections.Generic.List[object]]::new()
$ownedChildren = [System.Collections.Generic.List[object]]::new()
$seen = [System.Collections.Generic.HashSet[int]]::new()
foreach ($server in $servers) {
    $serverPid = [int]$server.ProcessId
    if (-not $seen.Add($serverPid)) { continue }
    $roots.Add((Identity $server 0))
    $queue = [System.Collections.Generic.Queue[object]]::new()
    $queue.Enqueue([pscustomobject]@{ pid = $serverPid; depth = 0 })
    while ($queue.Count -gt 0) {
        $entry = $queue.Dequeue()
        if (-not $children.ContainsKey([int]$entry.pid)) { continue }
        foreach ($childPid in $children[[int]$entry.pid]) {
            $child = $byPid[[int]$childPid]
            if ($null -eq $child -or -not $seen.Add([int]$childPid)) { continue }
            $depth = [int]$entry.depth + 1
            $ownedChildren.Add((Identity $child $depth))
            $queue.Enqueue([pscustomobject]@{ pid = [int]$childPid; depth = $depth })
        }
    }
}
[pscustomobject]@{
    roots = @($roots)
    children = @($ownedChildren)
} | ConvertTo-Json -Compress -Depth 6
]=]

require("mcdev.stdio").stop()
local jdtls_exit = rawget(_G, "mcdev_e2e_jdtls_exit")
local clients = vim.lsp.get_clients()
local snapshot
if vim.fn.has("win32") == 1 then
  local snapshot_result = powershell(snapshot_script, nil, 10000)
  local snapshot_error
  snapshot, snapshot_error = decode_snapshot(snapshot_result)
  if snapshot_error then
    abort_without_process_ownership(snapshot_error)
    return
  end
end

for _, client in ipairs(clients) do
  client:stop(false)
end

local function clients_stopped()
  for _, client in ipairs(clients) do
    if not client:is_stopped() then
      return false
    end
  end
  return true
end

local function process_exit_received()
  return jdtls_exit == nil or jdtls_exit.exited
end

local function settled()
  return clients_stopped() and process_exit_received()
end

local function wait_for_roots(snapshot_value, timeout_ms)
  if #snapshot_value.roots == 0 then
    return { rootsExited = true }
  end
  local wait_script = [=[
$ErrorActionPreference = 'Stop'
function CreationKey($process) {
    return $process.CreationDate.ToUniversalTime().ToString('o')
}
$payload = [Console]::In.ReadToEnd()
$snapshot = $payload | ConvertFrom-Json
$roots = @($snapshot.roots)
$deadline = [DateTime]::UtcNow.AddMilliseconds(]=] .. tostring(timeout_ms) .. [=[)
while ($true) {
    $alive = [System.Collections.Generic.List[object]]::new()
    foreach ($root in $roots) {
        try {
            $current = Get-CimInstance Win32_Process -Filter ("ProcessId = {0}" -f [int]$root.pid) -ErrorAction Stop
        } catch {
            [pscustomobject]@{ rootsExited = $false; error = $_.Exception.Message } | ConvertTo-Json -Compress
            exit 3
        }
        if ($null -ne $current -and (CreationKey $current) -eq [string]$root.creationDate) {
            $alive.Add($root)
        }
    }
    if ($alive.Count -eq 0) {
        [pscustomobject]@{ rootsExited = $true; alive = @() } | ConvertTo-Json -Compress
        exit 0
    }
    if ([DateTime]::UtcNow -ge $deadline) {
        [pscustomobject]@{ rootsExited = $false; alive = @($alive | ForEach-Object { [int]$_.pid }) } | ConvertTo-Json -Compress
        exit 2
    }
    Start-Sleep -Milliseconds 25
}
]=]
  local result = powershell(wait_script, vim.json.encode(snapshot_value), timeout_ms + 5000)
  if result == nil then
    return nil, "JDT process-root wait timed out"
  end
  local decoded_ok, decoded = pcall(vim.json.decode, result.stdout or "")
  if not decoded_ok or type(decoded) ~= "table" then
    return nil, "JDT process-root wait returned invalid JSON"
  end
  if decoded.rootsExited ~= true then
    return decoded, "JDT Java process did not exit before the graceful deadline"
  end
  return decoded
end

local function cleanup_owned(snapshot_value, force_roots)
  local force_literal = force_roots and "$true" or "$false"
  local cleanup_script = [=[
$ErrorActionPreference = 'Stop'
function CreationKey($process) {
    return $process.CreationDate.ToUniversalTime().ToString('o')
}
function CurrentProcess($entry) {
    return Get-CimInstance Win32_Process -Filter ("ProcessId = {0}" -f [int]$entry.pid) -ErrorAction Stop
}
function IsSameProcess($current, $entry) {
    return $null -ne $current -and
        (CreationKey $current) -eq [string]$entry.creationDate -and
        ([string]$current.Name) -eq [string]$entry.name
}
$payload = [Console]::In.ReadToEnd()
$snapshot = $payload | ConvertFrom-Json
$roots = @($snapshot.roots)
$children = @($snapshot.children | Sort-Object depth -Descending)
if (]=] .. force_literal .. [=[) {
    foreach ($root in $roots) {
        $current = CurrentProcess $root
        if ((IsSameProcess $current $root) -and
            ([string]$current.CommandLine) -like '*-Declipse.application=org.eclipse.jdt.ls.core.id1*') {
            $ErrorActionPreference = 'Continue'
            & taskkill.exe /PID ([int]$root.pid) /T /F *> $null
            $ErrorActionPreference = 'Stop'
        }
    }
}
if (]=] .. force_literal .. [=[) {
    $targets = @($roots) + @($children)
} else {
    $targets = @($children)
}
for ($attempt = 0; $attempt -lt 40; $attempt++) {
    $remaining = [System.Collections.Generic.List[int]]::new()
    foreach ($entry in $targets) {
        $current = CurrentProcess $entry
        if (-not (IsSameProcess $current $entry)) { continue }
        # A validated process may exit before taskkill. Its rechecked identity,
        # not taskkill stderr, determines whether cleanup succeeded.
        $ErrorActionPreference = 'Continue'
        & taskkill.exe /PID ([int]$entry.pid) /F *> $null
        $ErrorActionPreference = 'Stop'
        $current = CurrentProcess $entry
        if (IsSameProcess $current $entry) { $remaining.Add([int]$entry.pid) }
    }
    if ($remaining.Count -eq 0) {
        [pscustomobject]@{ cleaned = $true; remaining = @() } | ConvertTo-Json -Compress
        exit 0
    }
    Start-Sleep -Milliseconds 50
}
[pscustomobject]@{ cleaned = $false; remaining = @($remaining) } | ConvertTo-Json -Compress
exit 2
]=]
  local result = powershell(cleanup_script, vim.json.encode(snapshot_value), 5000)
  if result == nil then
    return nil, "JDT owned-process cleanup timed out"
  end
  local decoded_ok, decoded = pcall(vim.json.decode, result.stdout or "")
  if not decoded_ok or type(decoded) ~= "table" then
    return nil, "JDT owned-process cleanup returned invalid JSON: " .. tostring(result.stderr or result.stdout)
  end
  if decoded.cleaned ~= true or result.code ~= 0 then
    return decoded, "JDT owned-process cleanup left validated processes running"
  end
  return decoded
end

local stopped = false
local process_wait_error
local force_cleanup_succeeded = false
if snapshot then
  local waited, wait_error = wait_for_roots(snapshot, 15000)
  if waited and waited.rootsExited == true then
    local _, cleanup_error = cleanup_owned(snapshot, false)
    if cleanup_error then
      append_error(cleanup_error)
    end
    local settled_after_cleanup = wait(2000, settled, 50)
    stopped = cleanup_error == nil and settled_after_cleanup
    if not stopped and not cleanup_error then
      process_wait_error = "JDT process exited but its LSP transport did not settle after owned children were cleaned"
    end
  else
    process_wait_error = wait_error or "JDT Java process did not exit before the graceful deadline"
  end
else
  stopped = wait(15000, settled, 50)
end

if not stopped then
  if process_wait_error then
    append_error(process_wait_error)
  end
  if snapshot then
    local _, cleanup_error = cleanup_owned(snapshot, true)
    if cleanup_error then
      append_error(cleanup_error)
    else
      force_cleanup_succeeded = true
    end
  end
  -- The process tree is collected and force-cleaned before this escalation.
  for _, client in ipairs(clients) do
    client:stop(true)
  end
  if force_cleanup_succeeded then
    append_error("JDT graceful shutdown exceeded 15 seconds; stopped its owned process tree")
  else
    append_error("JDT graceful shutdown exceeded 15 seconds; could not verify its owned process tree was stopped")
  end
end

if jdtls_exit and jdtls_exit.exited
    and (jdtls_exit.code ~= 0 or jdtls_exit.signal ~= 0) then
  append_error(string.format(
    "JDT process exited abnormally (code=%s, signal=%s)",
    tostring(jdtls_exit.code),
    tostring(jdtls_exit.signal)
  ))
end
rawset(_G, "mcdev_e2e_jdtls_exit", nil)

if not ok then
  io.stderr:write(err .. "\n")
  vim.cmd("cquit 1")
else
  vim.cmd("qa!")
end
