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

local function buffer_text(bufnr)
  return table.concat(vim.api.nvim_buf_get_lines(bufnr, 0, -1, false), "\n")
end

function M.is_mcdev_completion_context(bufnr)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if M.detect_file_type(bufnr) ~= nil then
    return true
  end

  local filetype = vim.bo[bufnr].filetype
  local name = uri_basename(vim.uri_from_bufnr(bufnr))
  local text = buffer_text(bufnr)
  if filetype == "java" then
    if name:find("mixin", 1, true) ~= nil
      or text:find("org.spongepowered.asm.mixin", 1, true) ~= nil
      or text:find("com.llamalad7.mixinextras", 1, true) ~= nil
    then
      return true
    end
    for _, annotation in ipairs({
      "Mixin", "Inject", "At", "Shadow", "Accessor", "Invoker", "Overwrite", "Redirect", "Modify",
    }) do
      if text:find("@" .. annotation, 1, true) ~= nil then
        return true
      end
    end
    return false
  end
  if filetype == "json" then
    local mixin_named = name:match("mixins%.json$") ~= nil
      or name:match("mixins%.json5$") ~= nil
      or name:match("%.mixins%.json$") ~= nil
      or name:match("%.mixins%.json5$") ~= nil
    local mixin_shape = text:match('"package"%s*:') ~= nil
      and (text:match('"mixins"%s*:') ~= nil or text:match('"client"%s*:') ~= nil or text:match('"server"%s*:') ~= nil)
    return mixin_named or mixin_shape
  end
  return false
end

return M
