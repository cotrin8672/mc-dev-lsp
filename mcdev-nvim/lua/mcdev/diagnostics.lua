local buffer = require("mcdev.buffer")
local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")

local M = {}

M.namespace = vim.api.nvim_create_namespace("mcdev")

local debounce_timers = {}
local in_flight = {}
local augroup = nil

M.last_request = nil
M.last_error = nil
M.last_dropped_stale = false
M.running = false

function M.publish(bufnr, diagnostics)
  local vim_diagnostics = {}
  for _, diagnostic in ipairs(diagnostics or {}) do
    table.insert(vim_diagnostics, convert.to_vim_diagnostic(diagnostic))
  end
  vim.diagnostic.set(M.namespace, bufnr, vim_diagnostics)
end

local function changedtick(bufnr)
  if not vim.api.nvim_buf_is_valid(bufnr) then
    return nil
  end
  return vim.api.nvim_buf_get_changedtick(bufnr)
end

function M.fetch(bufnr, position, cb)
  protocol.diagnostics(bufnr, position, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    if cb then
      cb((result and result.diagnostics) or {}, nil)
    end
  end)
end

function M.refresh(bufnr, opts)
  opts = opts or {}
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if not vim.api.nvim_buf_is_valid(bufnr) then
    return
  end
  if in_flight[bufnr] and opts.in_flight_policy ~= "queue" then
    return
  end
  local request_tick = changedtick(bufnr)
  M.last_request = {
    bufnr = bufnr,
    changedtick = request_tick,
    manual = opts.manual == true,
    started_at = vim.loop and vim.loop.hrtime() or nil,
  }
  M.last_error = nil
  M.last_dropped_stale = false
  in_flight[bufnr] = true
  M.fetch(bufnr, nil, function(diagnostics, err)
    in_flight[bufnr] = nil
    if err then
      M.last_error = tostring(err)
      return
    end
    if opts.stale_result_policy ~= "publish" and request_tick ~= changedtick(bufnr) then
      M.last_dropped_stale = true
      return
    end
    M.publish(bufnr, diagnostics)
  end)
end

local function should_skip_insert_mode(opts)
  return not opts.insert_mode and vim.api.nvim_get_mode().mode:sub(1, 1) == "i"
end

function M.setup_autocmds(opts)
  opts = opts or {}
  local events = opts.events or { "BufWritePost" }
  local debounce_ms = opts.debounce_ms or 1000
  augroup = vim.api.nvim_create_augroup("McdevDiagnostics", { clear = true })
  M.running = true
  vim.api.nvim_create_autocmd(events, {
    group = augroup,
    callback = function(args)
      local bufnr = args.buf
      if not buffer.is_mcdev_buffer(bufnr) then
        return
      end
      if should_skip_insert_mode(opts) then
        return
      end
      if debounce_timers[bufnr] then
        vim.fn.timer_stop(debounce_timers[bufnr])
      end
      debounce_timers[bufnr] = vim.fn.timer_start(debounce_ms, function()
        debounce_timers[bufnr] = nil
        if vim.api.nvim_buf_is_valid(bufnr) then
          M.refresh(bufnr, opts)
        end
      end)
    end,
  })
end

function M.stop()
  if augroup then
    pcall(vim.api.nvim_del_augroup_by_id, augroup)
    augroup = nil
  end
  for bufnr, timer in pairs(debounce_timers) do
    vim.fn.timer_stop(timer)
    debounce_timers[bufnr] = nil
  end
  M.running = false
end

function M.start(opts)
  local config = require("mcdev.config")
  M.stop()
  M.setup_autocmds(opts or config.options.diagnostics)
end

function M.status_lines()
  local config = require("mcdev.config")
  local opts = config.options.diagnostics
  return {
    "mcdev diagnostics",
    "running: " .. tostring(M.running),
    "enabled: " .. tostring(opts.enabled),
    "events: " .. table.concat(opts.events or {}, ", "),
    "debounce_ms: " .. tostring(opts.debounce_ms),
    "insert_mode: " .. tostring(opts.insert_mode),
    "in_flight_buffers: " .. tostring(vim.tbl_count(in_flight)),
    "last_error: " .. tostring(M.last_error),
    "last_dropped_stale: " .. tostring(M.last_dropped_stale),
  }
end

return M
