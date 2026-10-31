-- Test-only synchronization and provenance for the unchanged editor scenarios.
local M = {}

function M.start(options)
  local acceptance_module = dofile(vim.fn.getcwd() .. '/mcdev-companion/tests/support/accepted_trace.lua')
  local state = {
    acceptance = acceptance_module.start(assert(options.trace), 'mcdev.mixin'),
    tracked = {}, records = {}, timeout = options.timeout or 60000,
    proof = { mode = 'real-host', provider = 'mcdev.mixin', scopePassed = false,
      indexing = { project = false, libraries = false }, acceptances = {}, commands = {},
      completions = {}, preflight = {}, originalAssertions = {}, shutdown = {} },
  }
  local frames = options.proof .. '.frames.jsonl'
  vim.fn.writefile({}, frames)
  function state:save()
    self.proof.error = self.failure
    if self.exit_state then self.proof.shutdown = vim.deepcopy(self.exit_state) end
    vim.fn.writefile({ vim.json.encode(self.proof) }, options.proof)
  end
  function state:fail(message)
    self.failure = tostring(message)
    self:save()
    error(self.failure, 0)
  end
  function state:log_message(message)
    if message:match('^ktlsp indexed %d+ project files') then self.proof.indexing.project = true end
    if message:match('^ktlsp indexed %d+ library files') then self.proof.indexing.libraries = true end
  end
  function state:wait_indexed()
    local ready = vim.wait(180000, function()
      return self.proof.indexing.project and self.proof.indexing.libraries
    end, 25)
    self.proof.indexing.ready = ready
    self:save()
    if not ready then self:fail('host indexing did not finish within 180s') end
  end
  function state:track(bufnr)
    if self.tracked[bufnr] then return end
    local tracked = { after = self.acceptance:mark() }
    self.tracked[bufnr] = tracked
    assert(vim.api.nvim_buf_attach(bufnr, false, {
      on_lines = function()
        if self.closed then return true end
        tracked.after = self.acceptance:mark()
      end,
      on_detach = function() self.tracked[bufnr] = nil end,
    }), 'could not track editor input changes')
  end
  function state:await(bufnr)
    self:track(bufnr)
    local text = vim.lsp._buf_get_full_text(bufnr)
    local tick = vim.api.nvim_buf_get_changedtick(bufnr)
    local owner = acceptance_module.owner_key(vim.uri_to_fname(vim.uri_from_bufnr(bufnr)))
    local expected = { [owner] = vim.fn.sha256(text) }
    require('vim.lsp._changetracking').flush(self.client)
    local ok, group = self.acceptance:wait_for(expected, self.tracked[bufnr].after,
      self.timeout, { required_instance = self.instance })
    if not ok then self:fail('host did not accept the exact current owner/source hash before the command') end
    if text ~= vim.lsp._buf_get_full_text(bufnr) or tick ~= vim.api.nvim_buf_get_changedtick(bufnr) then
      self:fail('editor source changed while waiting for host acceptance')
    end
    local accepted = assert(group[owner])
    self.instance = self.instance or accepted.instance
    self.proof.acceptances[#self.proof.acceptances + 1] = vim.deepcopy(accepted)
    self:save()
    return accepted
  end
  function state:attach(client, protocol, completion, exit_state)
    self.client, self.protocol, self.completion, self.exit_state = client, protocol, completion, exit_state
    self.original_request, self.original_complete = protocol.request, completion.complete
    self.dispatch = function(command, payload, callback, bufnr, accepted)
      local context = protocol.context(bufnr, { 1, 0 })
      local record = { command = command, bufnr = bufnr, context = context,
        sourceSha256 = vim.fn.sha256(context.bufferText), accepted = accepted, payload = vim.deepcopy(payload),
        index = #self.records + 1, acceptanceMark = self.acceptance:mark() }
      local summary = { command = command, commandIndex = record.index, documentUri = context.documentUri,
        sourceSha256 = record.sourceSha256, documentVersion = context.documentVersion,
        providerInstance = accepted.instance, acceptedEvaluation = accepted.evaluation,
        acceptedPublication = accepted.publication_revision, acceptedTraceMark = record.acceptanceMark, replied = false }
      self.records[#self.records + 1], self.proof.commands[#self.proof.commands + 1] = record, summary
      return self.original_request(command, payload, function(result, err, origin)
        record.result, record.error, record.replied = result, err, true
        local authentic = origin and origin.client == client and origin.client_id == client.id
          and origin.route == 'ktlsp.providerCommand' and origin.provider == 'mcdev.mixin' and origin.command == command
        record.authentic = authentic == true
        summary.replied, summary.authentic = true, record.authentic
        summary.origin = origin and { route = origin.route, provider = origin.provider, command = origin.command, clientId = origin.client_id } or nil
        summary.error = err and tostring(type(err) == 'table' and (err.message or err.code) or err) or nil
        summary.envelopeError = type(result) == 'table' and result.error or nil
        summary.resultSha256 = result and vim.fn.sha256(vim.json.encode(result)) or nil
        vim.fn.writefile({ vim.json.encode({ command = command, commandIndex = record.index, context = context,
          payload = record.payload, accepted = accepted, result = result, error = err,
          origin = summary.origin, authentic = record.authentic }) }, frames, 'a')
        if not authentic then self.failure = 'command response did not come from the dispatched native provider' end
        self:save()
        if callback then callback(result, err, origin, record) end
      end, bufnr)
    end
    protocol.request = function(command, payload, callback, bufnr)
      bufnr = (not bufnr or bufnr == 0) and vim.api.nvim_get_current_buf() or bufnr
      return self.dispatch(command, payload, callback, bufnr, self:await(bufnr))
    end
    completion.complete = function(callback, bufnr, position, opts)
      bufnr = bufnr or vim.api.nvim_get_current_buf()
      position = position or vim.api.nvim_win_get_cursor(0)
      self:await(bufnr)
      local context = protocol.context(bufnr, position)
      local source_hash = vim.fn.sha256(context.bufferText)
      return self.original_complete(function(value)
        if not value.isProvisional then
          local matched
          for index = #self.records, 1, -1 do
            local record = self.records[index]
            local result = record.result
            if record.command == protocol.commands.completion and record.bufnr == bufnr
              and record.sourceSha256 == source_hash and record.context.documentVersion == context.documentVersion
              and record.payload.context and vim.deep_equal(record.payload.context.position, context.position)
              and record.replied and record.authentic and not record.error and type(result) == 'table'
              and not result.error and type(result.result) == 'table' and type(result.result.items) == 'table' then
              local items = {}
              for _, item in ipairs(result.result.items) do
                local converted = completion.to_lsp_item(item)
                items[completion.item_key(converted)] = converted
              end
              matched = true
              for _, item in ipairs(value.items or {}) do
                if not vim.deep_equal(item, items[completion.item_key(item)]) then matched = false; break end
              end
              if matched then break end
            end
          end
          self.proof.completions[#self.proof.completions + 1] = {
            sourceSha256 = source_hash, documentVersion = context.documentVersion,
            position = context.position, items = #(value.items or {}), actualProviderReply = matched == true,
          }
          if not matched then self.failure = 'final completion lacks a matching actual provider result; helper fallback is not evidence' end
          self:save()
        end
        if callback then callback(value) end
      end, bufnr, position, opts)
    end
  end
  function state:preflight(bufnr)
    local started, backoff = vim.uv.hrtime(), 100
    local stale_retry, fresh_dispatch
    local function remaining_ms()
      return math.max(0, 60000 - (vim.uv.hrtime() - started) / 1e6)
    end
    local function same_context(record)
      local current = self.protocol.context(bufnr, { 1, 0 })
      return current.documentUri == record.context.documentUri
        and current.documentVersion == record.context.documentVersion
        and vim.fn.sha256(current.bufferText) == record.sourceSha256
    end
    while remaining_ms() > 0 do
      local attempt = { replied = false }
      self.proof.preflight[#self.proof.preflight + 1] = attempt
      local origin, dispatched
      local function receive(result, err, response_origin, record)
        attempt.result, attempt.error, attempt.replied = result, err, true
        origin, dispatched = response_origin, record
        attempt.accepted, attempt.documentVersion, attempt.commandIndex =
          vim.deepcopy(record.accepted), record.context.documentVersion, record.index
        if attempt.freshAcceptedRetry then stale_retry.afterCommandIndex = record.index end
      end
      local payload = { context = self.protocol.context(bufnr, { 1, 0 }) }
      local cancel
      if fresh_dispatch then
        if not same_context(fresh_dispatch.before) then self:fail('editor context changed before the fresh readiness retry') end
        attempt.freshAcceptedRetry = true
        cancel = self.dispatch(self.protocol.commands.info, payload, receive, bufnr, fresh_dispatch.accepted)
        fresh_dispatch = nil
      else
        cancel = self.protocol.request(self.protocol.commands.info, payload, receive, bufnr)
      end
      local replied = vim.wait(math.min(5000, remaining_ms()), function() return attempt.replied end, 25)
      if not replied then cancel(); self:fail('provider readiness request timed out') end
      if not origin or origin.client ~= self.client or origin.route ~= 'ktlsp.providerCommand'
        or origin.provider ~= 'mcdev.mixin' or origin.command ~= self.protocol.commands.info then
        self:fail('provider readiness reply has no authentic provider origin')
      end
      local err = attempt.error
      local unavailable = type(err) == 'table' and err.code == -32002
        and type(err.data) == 'table' and err.data.providerCommandReason == 'CommandUnavailable'
      if type(err) == 'table' and err.code == -32801 then
        if stale_retry then self:fail('provider readiness became Stale again after its one fresh retry') end
        if type(dispatched.context.documentVersion) ~= 'number' then self:fail('Stale readiness has no known editor version') end
        stale_retry = { initialError = vim.deepcopy(err), before = vim.deepcopy(dispatched.accepted),
          documentUri = dispatched.context.documentUri, sourceSha256 = dispatched.sourceSha256,
          documentVersion = dispatched.context.documentVersion, beforeCommandIndex = dispatched.index,
          traceAfter = dispatched.acceptanceMark, observedFreshAccepted = false }
        self.proof.preflightRetry = stale_retry
        attempt.retry = 'StaleAwaitFreshAccepted'
        self:save()
        local fresh, changed
        local owner = acceptance_module.owner_key(vim.uri_to_fname(dispatched.context.documentUri))
        local ready = vim.wait(remaining_ms(), function()
          assert(not self.acceptance.failure, self.acceptance.failure)
          if not same_context(dispatched) then changed = true; return true end
          for index = dispatched.acceptanceMark + 1, #self.acceptance.events do
            local event = self.acceptance.events[index]
            if event.instance == dispatched.accepted.instance and event.instance == self.instance
              and acceptance_module.owner_key(event.owner) == owner and event.source_sha256 == dispatched.sourceSha256
              and event.evaluation ~= dispatched.accepted.evaluation
              and event.publication_revision ~= dispatched.accepted.publication_revision then
              fresh = event
              return true
            end
          end
          return false
        end, 20)
        if changed then self:fail('editor context changed while waiting for fresh accepted readiness') end
        if not ready or not fresh then self:fail('Stale readiness had no fresh accepted evaluation within its deadline') end
        stale_retry.after, stale_retry.observedFreshAccepted = vim.deepcopy(fresh), true
        self.proof.acceptances[#self.proof.acceptances + 1] = vim.deepcopy(fresh)
        fresh_dispatch = { before = dispatched, accepted = fresh }
        self:save()
      elseif not unavailable then
        if err or type(attempt.result) ~= 'table' then self:fail('provider readiness RPC failed') end
        local envelope_error = attempt.result.error
        if envelope_error and (type(envelope_error) ~= 'table' or type(envelope_error.code) ~= 'string'
          or envelope_error.code:upper() ~= 'INCOMPLETE_PROJECT_CONTEXT') then
          self:fail('provider readiness returned an unexpected command failure')
        end
        if not envelope_error and type(attempt.result.result) ~= 'table' then
          self:fail('provider readiness response has no result envelope')
        end
        attempt.ready = true
        self:save()
        return
      else
        attempt.retry = 'CommandUnavailable'
        vim.wait(math.min(backoff, remaining_ms()), function() return false end, 25)
        backoff = math.min(backoff * 2, 1000)
      end
    end
    self:fail('provider readiness exceeded its 60s deadline')
  end
  function state:close()
    if self.closed then return end
    self.closed = true
    self.acceptance:close()
    if self.protocol then self.protocol.request = self.original_request end
    if self.completion then self.completion.complete = self.original_complete end
    self:save()
  end
  function state:run(callback)
    local original_record = _G.mcdev_record_assertion
    _G.mcdev_record_assertion = function(kind, caller)
      local source = caller and (caller.source or ''):gsub('^@', ''):gsub('\\', '/') or ''
      if source:match('/cem_project_e2e%.lua$') or source:match('/cem_feature_snippets%.lua$') then
        self.proof.originalAssertions[#self.proof.originalAssertions + 1] = { kind = kind, source = source, line = caller.currentline }
      end
      if original_record then original_record(kind, caller) end
    end
    self.proof.originalCemInvoked = true
    local ok, failure = xpcall(callback, debug.traceback)
    _G.mcdev_record_assertion = original_record
    if not ok then self.failure = tostring(failure) end
    self.proof.scopePassed = ok and self.failure == nil and #self.proof.completions > 0
      and #self.proof.originalAssertions > 0
    self:close()
    if not self.proof.scopePassed then error(self.failure or 'original CEM proof did not complete', 0) end
  end
  state.leave_autocmd = vim.api.nvim_create_autocmd('VimLeavePre', { once = true, callback = function()
    state:close()
    state:save()
  end })
  state:save()
  return state
end

return M
