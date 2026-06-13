local M = {}

local function uri_basename(uri)
  return (uri:match("([^/]+)$") or uri):lower()
end

function M.detect_file_type(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local language_id = vim.bo[bufnr].filetype:lower()
  local document_uri = vim.uri_from_bufnr(bufnr)
  local path = uri_basename(document_uri)

  if language_id == "accesswidener" then
    return "access_widener"
  end
  if language_id == "accesstransformer" then
    return "access_transformer"
  end
  if path:match("%.accesswidener$") or path:match("%.aw$") then
    return "access_widener"
  end
  if path:match("_at%.cfg$") or path == "accesstransformer.cfg" or path:match("%.at$") then
    return "access_transformer"
  end
  return nil
end

function M.effective_language_id(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local file_type = M.detect_file_type(bufnr)
  if file_type == "access_widener" then
    return "accesswidener"
  end
  if file_type == "access_transformer" then
    return "accesstransformer"
  end
  return vim.bo[bufnr].filetype
end

function M.is_mcdev_buffer(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  local ft = vim.bo[bufnr].filetype
  if ft == "java" or ft == "json" then
    return true
  end
  return M.detect_file_type(bufnr) ~= nil
end

return M
