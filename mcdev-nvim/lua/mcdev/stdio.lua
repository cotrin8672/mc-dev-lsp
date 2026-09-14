local jdtls = require("mcdev.jdtls")

local M = {}

M.timeout_ms = 5000

local state = {
  job_id = nil,
  jar = nil,
  next_id = 0,
  pending = {},
  stdout = "",
}

local function noop()
end

local function stop_timer(timer)
  if timer and not timer:is_closing() then
    timer:stop()
    timer:close()
  end
end

local function response_error(response)
  local error = response.error
  if type(error) == "table" and type(error.message) == "string" and error.message ~= "" then
    return error.message
  end
  return "mcdev: stdio helper did not handle request"
end

local function request_key(id)
  return tostring(id)
end

local function fail_pending(message)
  local pending = state.pending
  state.pending = {}
  for _, request in pairs(pending) do
    stop_timer(request.timer)
    request.callback(nil, message)
  end
end

local function handle_response(line)
  local ok, response = pcall(vim.json.decode, line)
  if not ok
    or type(response) ~= "table"
    or (type(response.id) ~= "number" and type(response.id) ~= "string")
    or type(response.handled) ~= "boolean"
  then
    fail_pending("mcdev: invalid stdio helper response")
    return
  end

  local key = request_key(response.id)
  local request = state.pending[key]
  if not request then
    return
  end
  state.pending[key] = nil
  stop_timer(request.timer)
  request.timer = nil

  if response.handled ~= true then
    request.callback(nil, response_error(response))
    return
  end
  if type(response.response) ~= "table" then
    request.callback(nil, "mcdev: stdio helper response is missing its payload")
    return
  end
  request.callback(response.response, nil)
end

local function on_stdout(job_id, data)
  if state.job_id ~= job_id then
    return
  end
  if type(data) ~= "table" then
    return
  end
  state.stdout = state.stdout .. table.concat(data, "\n")
  while true do
    local newline = state.stdout:find("\n", 1, true)
    if not newline then
      break
    end
    local line = state.stdout:sub(1, newline - 1)
    state.stdout = state.stdout:sub(newline + 1)
    if line ~= "" then
      handle_response(line)
    end
  end
end

local function on_exit(job_id)
  if state.job_id ~= job_id then
    return
  end
  state.job_id = nil
  state.jar = nil
  state.stdout = ""
  fail_pending("mcdev: stdio helper exited")
end

local function java_command()
  local java_home = vim.env.JAVA_HOME
  if java_home and java_home ~= "" then
    local suffix = package.config:sub(1, 1) == "\\" and "/bin/java.exe" or "/bin/java"
    local candidate = java_home:gsub("[\\/]$", "") .. suffix
    if vim.fn.executable(candidate) == 1 then
      return candidate
    end
  end
  local java = vim.fn.exepath("java")
  if java and java ~= "" then
    return java
  end
  if vim.fn.executable("java") == 1 then
    return "java"
  end
  return nil
end

function M.start(opts)
  opts = opts or {}
  local jar = jdtls.resolve_extension_jar(opts)
  if not jar or vim.fn.filereadable(jar) ~= 1 then
    return nil, "mcdev: extension jar is not readable"
  end

  if state.job_id and state.jar == jar then
    return state.job_id
  end
  if state.job_id then
    M.stop("mcdev: restarting stdio helper")
  end

  local java = opts.java
  if not java or java == "" then
    java = java_command()
  end
  if not java then
    return nil, "mcdev: java executable is not available"
  end

  local ok, job_id = pcall(vim.fn.jobstart, { java, "-cp", jar, "io.github.mcdev.jdtls.stdio.McdevStdioMain" }, {
    rpc = false,
    stdin = "pipe",
    stdout_buffered = false,
    stderr_buffered = false,
    on_stdout = on_stdout,
    on_exit = on_exit,
  })
  if not ok or type(job_id) ~= "number" or job_id <= 0 then
    return nil, "mcdev: failed to start stdio helper"
  end
  state.job_id = job_id
  state.jar = jar
  state.stdout = ""
  return job_id
end

function M.request(payload, callback, opts)
  opts = opts or {}
  local job_id, start_error = M.start(opts)
  if not job_id then
    return nil, start_error
  end

  state.next_id = state.next_id + 1
  local id = state.next_id
  local key = request_key(id)
  local request = {
    callback = callback or noop,
    timer = nil,
  }
  state.pending[key] = request

  request.timer = vim.defer_fn(function()
    if state.pending[key] ~= request then
      return
    end
    local message = "mcdev: stdio helper timed out"
    state.pending[key] = nil
    request.timer = nil
    M.stop(message)
    request.callback(nil, message)
  end, opts.timeout_ms or M.timeout_ms)

  local encoded = vim.json.encode({ id = id, payload = payload }) .. "\n"
  local ok, sent = pcall(vim.fn.chansend, job_id, encoded)
  if not ok or not sent or sent <= 0 then
    state.pending[key] = nil
    stop_timer(request.timer)
    request.timer = nil
    request.callback(nil, "mcdev: failed to write stdio helper request")
    return noop
  end

  return function()
    if state.pending[key] ~= request then
      return
    end
    state.pending[key] = nil
    stop_timer(request.timer)
    request.timer = nil
  end
end

function M.stop(reason)
  local job_id = state.job_id
  state.job_id = nil
  state.jar = nil
  state.stdout = ""
  fail_pending(reason or "mcdev: stdio helper stopped")
  if job_id then
    pcall(vim.fn.jobstop, job_id)
  end
end

return M
