local protocol = require("mcdev.protocol")

local M = {}

M.notification_method = "workspace/notify"
M.notification_command = "mcdev.transportReady"
M.timeout_ms = 5000

local uv = vim.uv or vim.loop
local endpoints = {}
local ready_listeners = {}
local ready_generations = {}
local active_requests = {}
local next_request_id = 0
local installed = false
local autocmd_group

local function notify_ready(client_id)
  ready_generations[client_id] = (ready_generations[client_id] or 0) + 1
  local listeners = ready_listeners[client_id] or {}
  ready_listeners[client_id] = nil
  for callback in pairs(listeners) do callback() end
end

local function close_handle(handle)
  if handle and not handle:is_closing() then
    handle:close()
  end
end

local function endpoint_valid(endpoint)
  return type(endpoint) == "table"
    and endpoint.protocolVersion == 1
    and type(endpoint.host) == "string"
    and endpoint.host == "127.0.0.1"
    and type(endpoint.port) == "number"
    and endpoint.port == math.floor(endpoint.port)
    and endpoint.port >= 1
    and endpoint.port <= 65535
    and type(endpoint.token) == "string"
    and #endpoint.token >= 32
end

local function endpoint_for(bufnr)
  local client = protocol.active_jdtls_client(bufnr)
  if not client or not client.id then
    return nil, nil
  end
  return endpoints[client.id], client.id
end

local function parse_response(raw)
  local header_end = raw:find("\r\n\r\n", 1, true)
  if not header_end then
    return nil, false
  end
  local header = raw:sub(1, header_end - 1)
  local body = raw:sub(header_end + 4)
  local status = tonumber(header:match("^HTTP/%d%.%d%s+(%d+)") or "0")
  local length = tonumber(header:lower():match("\r\ncontent%-length:%s*(%d+)") or "")
  if not status or not length then
    return nil, "mcdev: invalid completion transport response"
  end
  if #body < length then
    return nil, false
  end
  body = body:sub(1, length)
  local ok, decoded = pcall(vim.json.decode, body)
  if not ok or type(decoded) ~= "table" then
    return nil, "mcdev: invalid completion transport payload"
  end
  if status ~= 200 then
    local message = decoded.error and decoded.error.message or ("mcdev: transport HTTP " .. tostring(status))
    return nil, message
  end
  return decoded, true
end

function M.handle_notification(err, params, ctx)
  if err or type(params) ~= "table" or type(ctx) ~= "table" or not ctx.client_id then
    return false
  end
  local endpoint = params
  if params.command == M.notification_command then
    endpoint = params.arguments and params.arguments[1]
  elseif params[1] then
    -- JavaClientConnection.sendNotification uses LSP varargs, so
    -- the endpoint arrives as the first element of the params array.
    endpoint = params[1]
  end
  if not endpoint_valid(endpoint) then
    return false
  end
  endpoints[ctx.client_id] = vim.deepcopy(endpoint)
  notify_ready(ctx.client_id)
  return true
end

function M.ready_generation(bufnr)
  local client = protocol.active_jdtls_client(bufnr)
  return client and (ready_generations[client.id] or 0) or 0
end

function M.when_ready(bufnr, callback, after_generation)
  local client = protocol.active_jdtls_client(bufnr)
  if not client or not client.id then return function() end end
  local listeners = ready_listeners[client.id] or {}
  ready_listeners[client.id] = listeners
  local called = false
  local function wake()
    if called then return end
    called = true
    listeners[wake] = nil
    callback()
  end
  listeners[wake] = true
  if after_generation and (ready_generations[client.id] or 0) > after_generation then
    vim.schedule(function()
      if ready_listeners[client.id] == listeners then wake() end
    end)
  end
  return function() called = true; listeners[wake] = nil end
end

function M.endpoint(client_id)
  return endpoints[client_id]
end

function M.clear(client_id)
  endpoints[client_id] = nil
  ready_listeners[client_id] = nil
  ready_generations[client_id] = nil
  for _, request in pairs(active_requests) do
    if request.client_id == client_id then
      request.cancel()
    end
  end
end

function M.setup()
  if installed then
    return
  end
  installed = true
  local previous = vim.lsp.handlers[M.notification_method]
  vim.lsp.handlers[M.notification_method] = function(err, params, ctx)
    M.handle_notification(err, params, ctx)
    if previous then
      previous(err, params, ctx)
    end
  end
  local previous_status = vim.lsp.handlers["language/status"]
  vim.lsp.handlers["language/status"] = function(err, params, ctx)
    if not err and ctx and ctx.client_id and type(params) == "table"
      and (params.type == "Started" or params.type == "ServiceReady") then
      notify_ready(ctx.client_id)
    end
    if previous_status then previous_status(err, params, ctx) end
  end
  autocmd_group = vim.api.nvim_create_augroup("McdevCompletionTransport", { clear = true })
  vim.api.nvim_create_autocmd("LspDetach", {
    group = autocmd_group,
    callback = function(args)
      local client_id = args.data and args.data.client_id
      if client_id then
        M.clear(client_id)
      end
    end,
  })
  vim.api.nvim_create_autocmd("VimLeavePre", {
    group = autocmd_group,
    callback = function()
      M.stop()
    end,
  })
end

function M.request(payload, callback, bufnr, opts)
  opts = opts or {}
  callback = callback or function() end
  local endpoint, client_id = endpoint_for(bufnr or 0)
  if not endpoint then
    return nil, "mcdev: completion transport unavailable"
  end
  if not uv or not uv.new_tcp or not uv.new_timer then
    return nil, "mcdev: completion transport requires libuv"
  end

  local socket = uv.new_tcp()
  local timer = uv.new_timer()
  if not socket or not timer then
    close_handle(socket)
    close_handle(timer)
    return nil, "mcdev: failed to allocate completion transport handles"
  end

  next_request_id = next_request_id + 1
  local request_id = next_request_id
  local request
  local finished = false
  local raw = ""
  local body = vim.json.encode(payload)
  local wire = table.concat({
    "POST /completion HTTP/1.1\r\n",
    "Host: ", endpoint.host, "\r\n",
    "Authorization: Bearer ", endpoint.token, "\r\n",
    "Content-Type: application/json\r\n",
    "Content-Length: ", tostring(#body), "\r\n",
    "Connection: close\r\n\r\n",
    body,
  })

  local function finish(response, err, invoke_callback)
    if finished then
      return
    end
    finished = true
    active_requests[request_id] = nil
    if socket and not socket:is_closing() then
      pcall(socket.read_stop, socket)
    end
    close_handle(timer)
    close_handle(socket)
    if invoke_callback ~= false then
      vim.schedule(function() callback(response, err) end)
    end
  end

  request = {
    client_id = client_id,
    cancel = function()
      finish(nil, "mcdev: completion transport request cancelled", false)
    end,
  }
  active_requests[request_id] = request

  local timeout_ms = tonumber(opts.timeout_ms or endpoint.timeoutMs or M.timeout_ms) or M.timeout_ms
  timer:start(math.max(1, timeout_ms), 0, function()
    finish(nil, "mcdev: completion transport timed out")
  end)

  local function on_read(err, chunk)
    if finished then
      return
    end
    if err then
      finish(nil, "mcdev: completion transport read failed: " .. tostring(err))
      return
    end
    if chunk then
      raw = raw .. chunk
      local response, state = parse_response(raw)
      if state == true then
        finish(response, nil)
      elseif type(state) == "string" then
        finish(nil, state)
      end
    else
      finish(nil, "mcdev: completion transport closed before response")
    end
  end
  socket:connect(endpoint.host, endpoint.port, function(err)
    if finished then
      return
    end
    if err then
      finish(nil, "mcdev: completion transport connect failed: " .. tostring(err))
      return
    end
    socket:read_start(on_read)
    socket:write(wire, function(write_err)
      if write_err then
        finish(nil, "mcdev: completion transport write failed: " .. tostring(write_err))
      end
    end)
  end)

  return request.cancel
end

function M.stop()
  local requests = active_requests
  active_requests = {}
  for _, request in pairs(requests) do
    request.cancel()
  end
  endpoints = {}
  ready_listeners = {}
  ready_generations = {}
end

return M
