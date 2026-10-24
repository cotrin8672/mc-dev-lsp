-- Real standard host surface only; this does not claim MC command-wire coverage.
local h = dofile(vim.fn.getcwd() .. "/mcdev-nvim/tests/test_helpers.lua")
local binary = assert(vim.env.MCDEV_E2E_KTLSP, "supply the explicitly selected ktlsp executable")
h.assert_eq(vim.fn.executable(binary), 1)
local artifacts = vim.fn.getcwd() .. "/build/nvim-test-logs/materialized-f595"
local project = artifacts .. "/project-" .. vim.fn.getpid()
vim.fn.mkdir(project, "p")
local path = project .. "/DrillBlockMixinActionAlias.java"
local source = {
  "import java.util.Set;", "import java.util.List;", "",
  "class DrillBlockMixinActionAlias {", "    private int value;",
  "    private int sum(int left, int right) {", "        return left + right;", "    }", "}",
}
vim.fn.writefile(source, path)
local bufnr = vim.fn.bufadd(path)
vim.fn.bufload(bufnr)
vim.api.nvim_set_current_buf(bufnr)
vim.bo[bufnr].filetype = "java"
require("mcdev").setup()
local exited, exit_code, signal = false, nil, nil
local client, indexed, messages, raw, merged = nil, { project = false, libraries = false }, {}, nil, nil
local proof = { host = binary, before = table.concat(source, "\n"), scope = "standard materialized action; MC wire and extract-local remain pending" }
local ok, failure = xpcall(function()
  local id = assert(vim.lsp.start({ name = "ktlsp", cmd = { binary }, root_dir = project,
    on_exit = function(code, sig) exited, exit_code, signal = true, code, sig end,
    handlers = { ["window/logMessage"] = function(_, params)
      local message = params and params.message or ""
      messages[#messages + 1] = message
      if message:match("^ktlsp indexed %d+ project files") then indexed.project = true end
      if message:match("^ktlsp indexed %d+ library files") then indexed.libraries = true end
    end },
  }, { bufnr = bufnr }))
  h.assert_true(vim.wait(20000, function()
    client = vim.lsp.get_client_by_id(id)
    return client and client.initialized
  end, 20), "host must initialize")
  h.assert_true(vim.wait(40000, function() return indexed.project and indexed.libraries end, 20), "indexing must settle")
  local params = { textDocument = { uri = vim.uri_from_bufnr(bufnr) },
    range = { start = { line = 0, character = 0 }, ["end"] = { line = #source - 1, character = 1 } },
    context = { diagnostics = {} },
  }
  local response, err = client:request_sync("textDocument/codeAction", params, 10000, bufnr)
  h.assert_not_nil(response, tostring(err))
  h.assert_nil(response.err)
  raw = response.result or {}
  local native = vim.tbl_filter(function(action)
    return action.kind == "source.organizeImports" and action.edit
  end, raw)[1]
  h.assert_not_nil(native, "host must return its actual materialized organize-imports action")
  h.assert_eq(client.server_capabilities.codeActionProvider.resolveProvider, false)
  h.assert_nil(native.command)
  require("mcdev.lsp").code_actions(bufnr, params.range, {}, function(actions, request_error)
    h.assert_nil(request_error)
    merged = actions
  end)
  h.assert_true(vim.wait(10000, function() return merged ~= nil end, 20))
  local effects = vim.tbl_filter(function(action)
    return action.title == native.title and vim.deep_equal(action.edit, native.edit)
  end, merged)
  h.assert_eq(#effects, 1, "actual native effect appears once")
  h.assert_eq(effects[1]._mcdev_client_id, client.id)
  local expected = proof.before:gsub("import java.util.Set;\nimport java.util.List;", "import java.util.List;\nimport java.util.Set;")
  require("mcdev.code_action").apply(effects[1], bufnr)
  proof.after = table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
  h.assert_eq(proof.after, expected, "native edit must produce the complete expected buffer")
  require("mcdev.code_action").apply(effects[1], bufnr)
  h.assert_eq(table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n"), expected,
    "the now-stale action cannot apply its old edit again")
  proof.expected = expected
end, debug.traceback)
if client then client:stop(false) end
local stopped = vim.wait(20000, function() return exited end, 20)
if not stopped and client then client:stop(true); vim.wait(2000, function() return exited end, 20) end
proof.raw, proof.merged, proof.messages = raw, merged, messages
proof.shutdown = { exited = exited, code = exit_code, signal = signal }
proof.error = not ok and failure or nil
vim.fn.writefile({ vim.json.encode(proof) }, artifacts .. "/proof.json")
h.assert_true(ok, failure)
h.assert_true(stopped and exit_code == 0 and signal == 0, "the owned host must shut down cleanly")
print("real native materialized organize action, unique merged effect, exact applied buffer and stale rejection passed; MC wire/extract-local pending")
