local helpers = dofile(vim.fn.getcwd() .. '/mcdev-nvim/tests/test_helpers.lua')
local completion = require('mcdev.completion')
local protocol = require('mcdev.protocol')
local transport = require('mcdev.transport')
local stdio = require('mcdev.stdio')
local saved = {request=transport.request, ready=transport.when_ready, generation=transport.ready_generation,
  stdio=stdio.request, completion=protocol.completion, defer=vim.defer_fn}
local timeout_callback, timer_closed
vim.defer_fn = function(callback, timeout)
  helpers.assert_eq(timeout,120000)
  timeout_callback, timer_closed = callback, false
  return {stop=function() end,close=function() timer_closed=true end,is_closing=function() return timer_closed end}
end
local buf = vim.api.nvim_create_buf(false, true)
vim.bo[buf].filetype = 'java'
vim.api.nvim_buf_set_lines(buf,0,-1,false,{'@Mixin(Item.class)', '@Inject(method = "")'})
local on_ready, project_callback, cancel_count
transport.ready_generation = function() return 1 end
transport.when_ready = function(_, callback)
  on_ready = callback
  return function() cancel_count = cancel_count + 1; on_ready = nil end
end
transport.request = function(_, callback)
  project_callback = callback
  return function() end
end
stdio.request = function(_, callback)
  callback({result={items={}}})
  return function() end
end
protocol.completion = function(callback)
  callback({error={code='incomplete_project_context',message='dependencies not ready'}})
  return function() end
end
local function begin()
  on_ready, project_callback, cancel_count = nil, nil, 0
  local results = {}
  local cancel = completion.complete(function(result) results[#results+1]=result end,buf,{2,18},{stream=true})
  project_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
  helpers.assert_eq(#results,1)
  helpers.assert_true(results[1].isProvisional)
  helpers.assert_not_nil(on_ready)
  helpers.assert_eq(cancel_count,0)
  return results, cancel
end
local results = begin()
on_ready()
project_callback({result={items={{label='isEnchantable',insertText='isEnchantable',metadata={source='mixin.injectMethod'}}}}})
helpers.assert_eq(#results,2)
helpers.assert_eq(results[2].items[1].insertText,'isEnchantable')
helpers.assert_true(not results[2].isProvisional)

-- Editing/cancelling while import is pending must not publish a stale result.
local cancelled_results, cancel = begin()
local stale_ready = on_ready
cancel()
helpers.assert_eq(cancel_count,1)
stale_ready()
helpers.assert_eq(#cancelled_results,1)

-- Before the loopback endpoint exists, the standard command can return the
-- same readiness error. It must leave the existing readiness listener alive.
local request_project = transport.request
local jdt_callback
transport.request = function() return nil, 'endpoint not available' end
protocol.completion = function(callback) jdt_callback=callback; return function() end end
local standard_results = {}
cancel_count = 0
completion.complete(function(value) standard_results[#standard_results+1]=value end,buf,{2,18},{stream=true})
jdt_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
helpers.assert_true(standard_results[1].isProvisional)
helpers.assert_eq(cancel_count,0)
transport.request = request_project
on_ready()
project_callback({result={items={{label='getEnchantmentValue',insertText='getEnchantmentValue',metadata={source='mixin.injectMethod'}}}}})
helpers.assert_eq(#standard_results,2)
helpers.assert_eq(standard_results[2].items[1].insertText,'getEnchantmentValue')
helpers.assert_nil(completion.last_project_transport_error)

local silent_results = {}
completion.complete(function(value) silent_results[#silent_results+1]=value end,buf,{2,18})
project_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
jdt_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
helpers.assert_eq(#silent_results,0,'non-stream clients must not receive provisional completion')
local late_ready = on_ready
timeout_callback()
helpers.assert_eq(#silent_results,1)
helpers.assert_true(silent_results[1].isIncomplete)
helpers.assert_true(completion.last_error:find('120 seconds',1,true) ~= nil)
helpers.assert_true(timer_closed)
late_ready()
helpers.assert_eq(#silent_results,1,'timed-out requests cannot publish a later result')

transport.when_ready = function() return nil end
local detached_results = {}
completion.complete(function(value) detached_results[#detached_results+1]=value end,buf,{2,18},{stream=true})
project_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
jdt_callback({error={code='incomplete_project_context',message='dependencies not ready'}})
helpers.assert_eq(#detached_results,1)
helpers.assert_true(not detached_results[1].isProvisional)
helpers.assert_eq(completion.last_error,'dependencies not ready')

transport.request, transport.when_ready, transport.ready_generation = saved.request, saved.ready, saved.generation
stdio.request, protocol.completion = saved.stdio, saved.completion
vim.defer_fn = saved.defer
vim.api.nvim_buf_delete(buf,{force=true})
print('mcdev-nvim import readiness tests passed')
