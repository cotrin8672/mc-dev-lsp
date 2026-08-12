local M = {}

local function normalized_lines(lines)
  local result = {}
  for _, line in ipairs(lines or {}) do
    result[#result + 1] = tostring(line)
  end
  return #result > 0 and result or { "(no output)" }
end

function M.show_report(title, lines)
  lines = normalized_lines(lines)
  if not vim.api.nvim_list_uis or #vim.api.nvim_list_uis() == 0 then
    vim.notify(table.concat(lines, "\n"), vim.log.levels.INFO, { title = title })
    return nil
  end

  local max_width = 1
  for _, line in ipairs(lines) do
    max_width = math.max(max_width, vim.fn.strdisplaywidth(line))
  end
  local width = math.max(40, math.min(max_width, math.floor(vim.o.columns * 0.8)))
  local height = math.max(1, math.min(#lines, math.floor(vim.o.lines * 0.8)))
  local bufnr = vim.api.nvim_create_buf(false, true)
  vim.api.nvim_buf_set_lines(bufnr, 0, -1, false, lines)
  vim.bo[bufnr].buftype = "nofile"
  vim.bo[bufnr].bufhidden = "wipe"
  vim.bo[bufnr].swapfile = false
  vim.bo[bufnr].filetype = "mcdev"
  vim.bo[bufnr].modifiable = false

  local win = vim.api.nvim_open_win(bufnr, true, {
    relative = "editor",
    row = math.max(0, math.floor((vim.o.lines - height) / 2) - 1),
    col = math.max(0, math.floor((vim.o.columns - width) / 2)),
    width = width,
    height = height,
    style = "minimal",
    border = "rounded",
    title = " " .. title .. " ",
    title_pos = "center",
  })
  local function close()
    if vim.api.nvim_win_is_valid(win) then
      vim.api.nvim_win_close(win, true)
    end
  end
  vim.keymap.set("n", "q", close, { buffer = bufnr, silent = true })
  vim.keymap.set("n", "<Esc>", close, { buffer = bufnr, silent = true })
  return win
end

function M.error(message)
  message = tostring(message)
  if not message:match("^mcdev:") then
    message = "mcdev: " .. message
  end
  vim.notify(message, vim.log.levels.WARN)
end

return M
