local M = {}

local mcdev_annotation_names = {
  Mixin = true,
  Inject = true,
  At = true,
  Shadow = true,
  Accessor = true,
  Invoker = true,
  Overwrite = true,
  Redirect = true,
  ModifyArg = true,
  ModifyArgs = true,
  ModifyVariable = true,
  ModifyConstant = true,
  ModifyExpressionValue = true,
  ModifyReturnValue = true,
  ModifyReceiver = true,
  WrapOperation = true,
  WrapWithCondition = true,
  WrapMethod = true,
  Wrap = true,
  Constant = true,
  Slice = true,
  Local = true,
  Share = true,
  Cancellable = true,
  Definition = true,
  Definitions = true,
  Expression = true,
  Expressions = true,
}

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

local function text_before_cursor(bufnr, position)
  local row = position and position[1] or vim.api.nvim_win_get_cursor(0)[1]
  local col = position and position[2] or vim.api.nvim_win_get_cursor(0)[2]
  local lines = vim.api.nvim_buf_get_lines(bufnr, 0, row, false)
  if #lines == 0 then
    return ""
  end
  lines[#lines] = lines[#lines]:sub(1, col)
  return table.concat(lines, "\n")
end

local function is_mcdev_annotation_prefix(name)
  local prefix = name:lower()
  for annotation in pairs(mcdev_annotation_names) do
    if annotation:sub(1, #prefix):lower() == prefix then
      return true
    end
  end
  return false
end

local function mcdev_annotation_context(text)
  local paren_depth = 0
  local annotation_depths = {}
  local has_annotation_prefix = false
  local quote = nil
  local escaped = false
  local line_comment = false
  local block_comment = false
  local index = 1

  while index <= #text do
    local char = text:sub(index, index)
    local next_char = text:sub(index + 1, index + 1)

    if line_comment then
      if char == "\n" then
        line_comment = false
      end
    elseif block_comment then
      if char == "*" and next_char == "/" then
        block_comment = false
        index = index + 1
      end
    elseif quote then
      if escaped then
        escaped = false
      elseif char == "\\" then
        escaped = true
      elseif char == quote then
        quote = nil
      end
    elseif char == '"' or char == "'" then
      quote = char
    elseif char == "/" and next_char == "/" then
      line_comment = true
      index = index + 1
    elseif char == "/" and next_char == "*" then
      block_comment = true
      index = index + 1
    elseif char == "@" then
      local name = text:match("^([%w_.$]+)", index + 1)
      if name then
        local name_end = index + #name
        local open = name_end + 1
        while text:sub(open, open):match("%s") do
          open = open + 1
        end
        local simple_name = name:match("([%w_]+)$")
        local prefix_name = text:match("^([%w_]+)", index + 1)
        local prefix_end = prefix_name and index + #prefix_name + 1 or nil
        if prefix_name
          and is_mcdev_annotation_prefix(prefix_name)
          and text:sub(prefix_end):match("^[%s]*$")
          and not text:sub(prefix_end):find("\n", 1, true)
        then
          has_annotation_prefix = true
        end
        if text:sub(open, open) == "(" and mcdev_annotation_names[simple_name] then
          annotation_depths[#annotation_depths + 1] = paren_depth + 1
        end
      elseif index == #text then
        has_annotation_prefix = true
      end
    elseif char == "(" then
      paren_depth = paren_depth + 1
    elseif char == ")" then
      paren_depth = math.max(paren_depth - 1, 0)
      while #annotation_depths > 0 and annotation_depths[#annotation_depths] > paren_depth do
        table.remove(annotation_depths)
      end
    end

    index = index + 1
  end

  return #annotation_depths > 0, has_annotation_prefix
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
      "ModifyArg",
      "ModifyArgs", "ModifyVariable", "ModifyConstant", "ModifyExpressionValue", "ModifyReturnValue",
      "ModifyReceiver", "WrapOperation", "WrapWithCondition", "WrapMethod", "Wrap", "Constant", "Slice", "Local",
      "Share", "Cancellable", "Definition", "Definitions", "Expression", "Expressions",
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

function M.is_mcdev_completion_context_at(bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if M.detect_file_type(bufnr) ~= nil then
    return true
  end

  local filetype = vim.bo[bufnr].filetype
  if filetype == "json" then
    return M.is_mcdev_completion_context(bufnr)
  end
  if filetype ~= "java" then
    return false
  end

  local has_open_annotation, has_annotation_prefix = mcdev_annotation_context(text_before_cursor(bufnr, position))
  return has_open_annotation or has_annotation_prefix
end

return M
