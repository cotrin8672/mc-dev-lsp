local files = vim.fn.glob("mcdev-nvim/**/*.lua", false, true)
table.sort(files)
for _, path in ipairs(files) do assert(loadfile(path), "invalid Lua syntax: " .. path) end
print("parsed " .. #files .. " client and test Lua files without executing E2E or starting a server")
