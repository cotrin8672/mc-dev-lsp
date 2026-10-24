vim.opt.runtimepath:prepend(vim.fn.getcwd() .. "/mcdev-nvim")
local evidence = {}
_G.mcdev_record_assertion = function(kind, caller)
  if not caller then return end
  local source = (caller.source or "?"):gsub("^@", ""):gsub("\\", "/")
  local start = source:find("mcdev-nvim/tests/", 1, true)
  if not start then return end
  local key = source:sub(start) .. ":" .. caller.currentline .. ":" .. kind
  evidence[key] = (evidence[key] or 0) + 1
end
local original_assert = assert
_G.assert = function(...)
  mcdev_record_assertion("assert", debug.getinfo(2, "Sl"))
  return original_assert(...)
end
vim.api.nvim_create_autocmd("VimLeavePre", {
  callback = function()
    local path = vim.env.MCDEV_ASSERTION_LOG
    if path and path ~= "" then
      vim.fn.writefile({ vim.json.encode(evidence) }, path)
      local modules = {}
      for name in pairs(package.loaded) do
        if name == "mcdev" or name:match("^mcdev%.") then modules[#modules + 1] = name end
      end
      table.sort(modules)
      local modules_path = (path:gsub("%.assertions%.json$", ".modules.json"))
      vim.fn.writefile({ vim.json.encode(modules) }, modules_path)
    end
  end,
})
