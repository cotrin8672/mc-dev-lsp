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
  Group = true,
  Debug = true,
  Dynamic = true,
  Unique = true,
  Intrinsic = true,
  Implements = true,
  Interface = true,
  Desc = true,
  Descriptors = true,
}

local mcdev_annotation_fqns = {
  ["org.spongepowered.asm.mixin.Mixin"] = "Mixin",
  ["org.spongepowered.asm.mixin.Shadow"] = "Shadow",
  ["org.spongepowered.asm.mixin.gen.Accessor"] = "Accessor",
  ["org.spongepowered.asm.mixin.gen.Invoker"] = "Invoker",
  ["org.spongepowered.asm.mixin.injection.Inject"] = "Inject",
  ["org.spongepowered.asm.mixin.injection.Redirect"] = "Redirect",
  ["org.spongepowered.asm.mixin.injection.ModifyArg"] = "ModifyArg",
  ["org.spongepowered.asm.mixin.injection.ModifyArgs"] = "ModifyArgs",
  ["org.spongepowered.asm.mixin.injection.ModifyVariable"] = "ModifyVariable",
  ["org.spongepowered.asm.mixin.injection.ModifyConstant"] = "ModifyConstant",
  ["com.llamalad7.mixinextras.injector.ModifyExpressionValue"] = "ModifyExpressionValue",
  ["com.llamalad7.mixinextras.injector.ModifyReturnValue"] = "ModifyReturnValue",
  ["com.llamalad7.mixinextras.injector.ModifyReceiver"] = "ModifyReceiver",
  ["com.llamalad7.mixinextras.injector.wrapoperation.WrapOperation"] = "WrapOperation",
  ["com.llamalad7.mixinextras.injector.WrapWithCondition"] = "WrapWithCondition",
  ["com.llamalad7.mixinextras.injector.v2.WrapWithCondition"] = "WrapWithCondition",
  ["com.llamalad7.mixinextras.injector.wrapmethod.WrapMethod"] = "WrapMethod",
  ["org.spongepowered.asm.mixin.Overwrite"] = "Overwrite",
  ["org.spongepowered.asm.mixin.injection.At"] = "At",
  ["org.spongepowered.asm.mixin.injection.Constant"] = "Constant",
  ["org.spongepowered.asm.mixin.injection.Slice"] = "Slice",
  ["com.llamalad7.mixinextras.sugar.Local"] = "Local",
  ["com.llamalad7.mixinextras.sugar.Share"] = "Share",
  ["com.llamalad7.mixinextras.expression.Definition"] = "Definition",
  ["com.llamalad7.mixinextras.expression.Definitions"] = "Definitions",
  ["com.llamalad7.mixinextras.expression.Expression"] = "Expression",
  ["com.llamalad7.mixinextras.expression.Expressions"] = "Expressions",
  ["org.spongepowered.asm.mixin.injection.Group"] = "Group",
  ["org.spongepowered.asm.mixin.Debug"] = "Debug",
  ["org.spongepowered.asm.mixin.Dynamic"] = "Dynamic",
  ["org.spongepowered.asm.mixin.Unique"] = "Unique",
  ["org.spongepowered.asm.mixin.Intrinsic"] = "Intrinsic",
  ["org.spongepowered.asm.mixin.Implements"] = "Implements",
  ["org.spongepowered.asm.mixin.Interface"] = "Interface",
  ["org.spongepowered.asm.mixin.injection.Desc"] = "Desc",
  ["org.spongepowered.asm.mixin.injection.Descriptors"] = "Descriptors",
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

local function text_after_cursor(bufnr, position)
  local row = position and position[1] or vim.api.nvim_win_get_cursor(0)[1]
  local col = position and position[2] or vim.api.nvim_win_get_cursor(0)[2]
  local lines = vim.api.nvim_buf_get_lines(bufnr, row - 1, -1, false)
  if #lines == 0 then
    return ""
  end
  lines[1] = lines[1]:sub(col + 1)
  return table.concat(lines, "\n")
end

local function simple_annotation_name(name)
  return name:match("([%w_]+)$") or name
end

local function explicit_annotation_imports(text)
  local imports = {}
  for line in text:gmatch("[^\n]+") do
    local fqn = line:match("^%s*import%s+([%w_.$]+)%s*;")
    if fqn then
      imports[simple_annotation_name(fqn)] = fqn
    end
  end
  return imports
end

local function resolved_annotation_name(name, imports)
  local simple = simple_annotation_name(name)
  if name:find(".", 1, true) then
    return mcdev_annotation_fqns[name]
  end
  if imports and imports[simple] then
    return mcdev_annotation_fqns[imports[simple]]
  end
  return mcdev_annotation_names[simple] and simple or nil
end

local function is_mcdev_annotation_prefix(name, imports)
  local prefix = name:lower()
  if name:find(".", 1, true) then
    for fqn in pairs(mcdev_annotation_fqns) do
      if fqn:sub(1, #name):lower() == prefix then
        return true
      end
    end
    return false
  end
  if imports and imports[name] then
    return mcdev_annotation_fqns[imports[name]] ~= nil
  end
  for annotation in pairs(mcdev_annotation_names) do
    if annotation:sub(1, #prefix):lower() == prefix then
      return true
    end
  end
  return false
end

-- The Java server knows the complete annotation grammar, but Blink has to
-- choose its providers before it can ask the server.  Keep this table small:
-- it only describes values where a normal Java completion is misleading.
-- Class literals, enum constants, booleans, numbers, and nested annotation
-- names stay mixed with JDT completion.
local annotation_attribute_modes = {
  Mixin = { targets = "exclusive", target = "exclusive" },
  Inject = { method = "exclusive" },
  Redirect = { method = "exclusive" },
  ModifyArg = { method = "exclusive" },
  ModifyArgs = { method = "exclusive" },
  ModifyVariable = { method = "exclusive", name = "exclusive" },
  ModifyConstant = { method = "exclusive" },
  ModifyExpressionValue = { method = "exclusive" },
  ModifyReturnValue = { method = "exclusive" },
  ModifyReceiver = { method = "exclusive" },
  WrapOperation = { method = "exclusive" },
  WrapWithCondition = { method = "exclusive" },
  WrapMethod = { method = "exclusive" },
  Wrap = { value = "exclusive" },
  At = { value = "exclusive", target = "exclusive", args = "exclusive" },
  Slice = {},
  Accessor = { value = "exclusive" },
  Invoker = { value = "exclusive" },
  Shadow = { prefix = "exclusive" },
  Local = { value = "exclusive", name = "exclusive" },
  Share = { value = "exclusive", namespace = "exclusive" },
  Definition = { value = "exclusive", id = "exclusive", method = "exclusive", field = "exclusive" },
  Definitions = {},
  Expression = { value = "exclusive", id = "exclusive" },
  Expressions = { value = "exclusive" },
  Constant = { stringValue = "exclusive" },
}

-- These annotations have completion snippets for their names, but no custom
-- attribute/value completion. Keep their bodies available to JDT LS.
local name_only_annotations = {
  Group = true,
  Debug = true,
  Dynamic = true,
  Unique = true,
  Intrinsic = true,
  Implements = true,
  Interface = true,
  Desc = true,
  Descriptors = true,
}

local function trailing_annotation_prefix(text, scan, imports)
  if scan.quote or scan.line_comment or scan.block_comment or scan.char_literal then
    return nil
  end
  local at = text:find("@[%w_.$]*%s*$")
  if not at or (at > 1 and text:sub(at - 1, at - 1):match("[%w_.$]")) then
    return nil
  end
  local token = text:sub(at + 1):match("^([%w_.$]*)") or ""
  if is_mcdev_annotation_prefix(token, imports) then
    return token
  end
  return nil
end

local function scan_open_annotations(text, imports)
  local frames = {}
  local completed_annotations = {}
  local paren_depth = 0
  local pending_annotation
  local quote = nil
  local char_literal = false
  local escaped = false
  local line_comment = false
  local block_comment = false
  local index = 1

  while index <= #text do
    local char = text:sub(index, index)
    local next_char = text:sub(index + 1, index + 1)

    if line_comment then
      if char == "\n" then line_comment = false end
    elseif block_comment then
      if char == "*" and next_char == "/" then
        block_comment = false
        index = index + 1
      end
    elseif quote or char_literal then
      if escaped then
        escaped = false
      elseif char == "\\" then
        escaped = true
      elseif (quote and char == quote) or (char_literal and char == "'") then
        quote = nil
        char_literal = false
      end
    elseif char == '"' then
      quote = char
      pending_annotation = nil
    elseif char == "'" then
      char_literal = true
      pending_annotation = nil
    elseif char == "/" and next_char == "/" then
      line_comment = true
      pending_annotation = nil
      index = index + 1
    elseif char == "/" and next_char == "*" then
      block_comment = true
      pending_annotation = nil
      index = index + 1
    elseif char == "@" then
      if pending_annotation and pending_annotation.recognized then
        completed_annotations[#completed_annotations + 1] = {
          name = pending_annotation.name,
          at = pending_annotation.at,
          close = pending_annotation.name_end,
        }
      end
      local name = text:match("^([%w_.$]*)", index + 1) or ""
      local name_end = index + #name
      local resolved = resolved_annotation_name(name, imports)
      pending_annotation = {
        name = resolved or simple_annotation_name(name),
        recognized = resolved ~= nil,
        at = index,
        name_end = name_end,
      }
      index = name_end
    elseif char == "(" then
      paren_depth = paren_depth + 1
      if pending_annotation and text:sub(pending_annotation.name_end + 1, index - 1):match("^%s*$")
        and pending_annotation.recognized
      then
        frames[#frames + 1] = {
          name = pending_annotation.name,
          open = index,
          depth = paren_depth,
        }
      end
      pending_annotation = nil
    elseif char == ")" then
      paren_depth = math.max(paren_depth - 1, 0)
      while #frames > 0 and frames[#frames].depth > paren_depth do
        local frame = table.remove(frames)
        frame.close = index
        completed_annotations[#completed_annotations + 1] = frame
      end
      pending_annotation = nil
    elseif pending_annotation and not char:match("%s") then
      if pending_annotation.recognized then
        completed_annotations[#completed_annotations + 1] = {
          name = pending_annotation.name,
          at = pending_annotation.at,
          close = pending_annotation.name_end,
        }
      end
      pending_annotation = nil
    end

    index = index + 1
  end

  local scan = {
    frames = frames,
    completed_annotations = completed_annotations,
    quote = quote,
    char_literal = char_literal,
    line_comment = line_comment,
    block_comment = block_comment,
  }
  scan.annotation_prefix = trailing_annotation_prefix(text, scan, imports)
  return scan
end

local member_modifiers = {
  public = true,
  protected = true,
  private = true,
  static = true,
  final = true,
  abstract = true,
  synchronized = true,
  native = true,
  strictfp = true,
  default = true,
  sealed = true,
  non = true,
}

local function mask_member_annotations(text)
  local chars = {}
  for index = 1, #text do
    chars[index] = text:sub(index, index)
  end

  local quote = nil
  local char_literal = false
  local escaped = false
  local line_comment = false
  local block_comment = false
  local index = 1
  while index <= #text do
    local char = text:sub(index, index)
    local next_char = text:sub(index + 1, index + 1)
    if line_comment then
      if char == "\n" then line_comment = false end
    elseif block_comment then
      if char == "*" and next_char == "/" then
        block_comment = false
        index = index + 1
      end
    elseif quote or char_literal then
      if escaped then
        escaped = false
      elseif char == "\\" then
        escaped = true
      elseif (quote and char == quote) or (char_literal and char == "'") then
        quote = nil
        char_literal = false
      end
    elseif char == '"' then
      quote = char
    elseif char == "'" then
      char_literal = true
    elseif char == "/" and next_char == "/" then
      line_comment = true
      index = index + 1
    elseif char == "/" and next_char == "*" then
      block_comment = true
      index = index + 1
    elseif char == "@" then
      local name_end = index + 1
      while name_end <= #text and text:sub(name_end, name_end):match("[%w_.$]") do
        name_end = name_end + 1
      end
      local argument_start = name_end
      while argument_start <= #text and text:sub(argument_start, argument_start):match("%s") do
        argument_start = argument_start + 1
      end

      local annotation_end = name_end - 1
      if text:sub(argument_start, argument_start) == "(" then
        local depth = 0
        local argument_quote = nil
        local argument_char = false
        local argument_escaped = false
        local argument_line_comment = false
        local argument_block_comment = false
        local cursor = argument_start
        while cursor <= #text do
          local current = text:sub(cursor, cursor)
          local following = text:sub(cursor + 1, cursor + 1)
          if argument_line_comment then
            if current == "\n" then argument_line_comment = false end
          elseif argument_block_comment then
            if current == "*" and following == "/" then
              argument_block_comment = false
              cursor = cursor + 1
            end
          elseif argument_quote or argument_char then
            if argument_escaped then
              argument_escaped = false
            elseif current == "\\" then
              argument_escaped = true
            elseif (argument_quote and current == argument_quote)
              or (argument_char and current == "'")
            then
              argument_quote = nil
              argument_char = false
            end
          elseif current == '"' then
            argument_quote = current
          elseif current == "'" then
            argument_char = true
          elseif current == "/" and following == "/" then
            argument_line_comment = true
            cursor = cursor + 1
          elseif current == "/" and following == "*" then
            argument_block_comment = true
            cursor = cursor + 1
          elseif current == "(" then
            depth = depth + 1
          elseif current == ")" then
            depth = depth - 1
            if depth == 0 then
              annotation_end = cursor
              break
            end
          end
          cursor = cursor + 1
        end
        if annotation_end == name_end - 1 then
          annotation_end = #text
        end
      end

      for masked = index, annotation_end do
        chars[masked] = " "
      end
      index = annotation_end
    end
    index = index + 1
  end
  return table.concat(chars)
end

local function first_member_delimiter(text)
  text = mask_member_annotations(text)
  local quote = nil
  local char_literal = false
  local escaped = false
  local line_comment = false
  local block_comment = false
  local index = 1

  while index <= #text do
    local char = text:sub(index, index)
    local next_char = text:sub(index + 1, index + 1)
    if line_comment then
      if char == "\n" then line_comment = false end
    elseif block_comment then
      if char == "*" and next_char == "/" then
        block_comment = false
        index = index + 1
      end
    elseif quote or char_literal then
      if escaped then
        escaped = false
      elseif char == "\\" then
        escaped = true
      elseif (quote and char == quote) or (char_literal and char == "'") then
        quote = nil
        char_literal = false
      end
    elseif char == '"' then
      quote = char
    elseif char == "'" then
      char_literal = true
    elseif char == "/" and next_char == "/" then
      line_comment = true
      index = index + 1
    elseif char == "/" and next_char == "*" then
      block_comment = true
      index = index + 1
    elseif char == "(" or char == "{" or char == ";" or char == "}" then
      return index, char
    end
    index = index + 1
  end
end

local function member_header_context(text, after, scan)
  local marker
  for index = #scan.completed_annotations, 1, -1 do
    local candidate = scan.completed_annotations[index]
    if candidate.name == "Overwrite" or candidate.name == "Shadow" then
      marker = candidate
      break
    end
  end
  if not marker then return nil end

  local tail = text:sub(marker.close + 1)
  local _, tail_delimiter = first_member_delimiter(tail)
  if tail_delimiter and not (marker.name == "Shadow" and tail_delimiter == ";") then
    return nil
  end

  local combined = tail .. after
  local delimiter_index, delimiter = first_member_delimiter(combined)
  local header
  if delimiter == "(" then
    if delimiter_index <= #tail then
      return nil
    end
    header = combined:sub(1, delimiter_index - 1)
  elseif delimiter == ";" and marker.name == "Shadow" then
    if delimiter_index <= #tail then
      return nil
    end
    header = combined:sub(1, delimiter_index - 1)
  elseif not delimiter and (marker.name == "Overwrite" or marker.name == "Shadow")
    and after:match("^[%w_$%s]*$")
  then
    header = combined
  else
    return nil
  end

  local masked_header = mask_member_annotations(header)
  local name_start = masked_header:find("[%a_$][%w_$]*%s*$")
  if not name_start then return nil end
  local name = masked_header:sub(name_start):match("^[%a_$][%w_$]*")
  if not name then return nil end
  local name_end = name_start + #name - 1

  local return_type_tokens = 0
  for token in masked_header:sub(1, name_start - 1):gmatch("[%a_$][%w_$]*") do
    if not member_modifiers[token] then
      return_type_tokens = return_type_tokens + 1
    end
  end
  if return_type_tokens == 0 then
    return nil
  end

  local cursor_offset = #tail
  if cursor_offset < name_start - 1 then
    return nil
  end
  if cursor_offset > name_end
    and not combined:sub(name_end + 1, cursor_offset):match("^%s*$")
  then
    return nil
  end

  return {
    owned = true,
    exclusive = marker.name ~= "Shadow" or delimiter == "(" or delimiter == ";",
    kind = "member",
    annotation = marker.name,
  }
end

local function current_annotation_segment(text, frame)
  local body = text:sub(frame.open + 1)
  local segment_start = 1
  local after_separator = false
  local paren_depth = 0
  local brace_depth = 0
  local quote = nil
  local char_literal = false
  local escaped = false
  local line_comment = false
  local block_comment = false
  local value_started = false
  local value_has_string = false
  local value_has_outside_text = false
  local index = 1

  while index <= #body do
    local char = body:sub(index, index)
    local next_char = body:sub(index + 1, index + 1)
    if line_comment then
      if char == "\n" then line_comment = false end
    elseif block_comment then
      if char == "*" and next_char == "/" then
        block_comment = false
        index = index + 1
      end
    elseif quote or char_literal then
      if escaped then
        escaped = false
      elseif char == "\\" then
        escaped = true
      elseif (quote and char == quote) or (char_literal and char == "'") then
        if quote and value_started then
          value_has_string = true
        end
        quote = nil
        char_literal = false
      end
    elseif char == '"' then
      if not value_started then
        value_started = true
      end
      quote = char
    elseif char == "'" then
      if not value_started then value_started = true end
      value_has_outside_text = true
      char_literal = true
    elseif char == "/" and next_char == "/" then
      line_comment = true
      index = index + 1
    elseif char == "/" and next_char == "*" then
      block_comment = true
      index = index + 1
    elseif char == "(" then
      if value_started then
        value_has_outside_text = true
      end
      paren_depth = paren_depth + 1
    elseif char == ")" then
      paren_depth = math.max(paren_depth - 1, 0)
    elseif char == "{" then
      if value_started and brace_depth > 0 then
        value_has_outside_text = true
      end
      brace_depth = brace_depth + 1
    elseif char == "}" then
      if value_started and brace_depth > 1 then
        value_has_outside_text = true
      end
      brace_depth = math.max(brace_depth - 1, 0)
    elseif char == "," then
      if paren_depth == 0 and brace_depth == 0 then
        segment_start = index + 1
        after_separator = true
        value_started = false
        value_has_string = false
        value_has_outside_text = false
      elseif value_started and brace_depth == 1 then
        value_has_string = false
        value_has_outside_text = false
      elseif value_started then
        value_has_outside_text = true
      end
    elseif char == "=" and paren_depth == 0 and brace_depth == 0 then
      if value_started then
        value_has_outside_text = true
      else
        value_started = true
      end
    elseif value_started and not char:match("%s") then
      value_has_outside_text = true
    end
    index = index + 1
  end

  local segment = body:sub(segment_start)
  while true do
    segment = segment:gsub("^%s+", "", 1)
    if segment:sub(1, 2) == "/*" then
      local close = segment:find("*/", 3, true)
      if not close then break end
      segment = segment:sub(close + 2)
    elseif segment:sub(1, 2) == "//" then
      local newline = segment:find("\n", 3, true)
      if not newline then break end
      segment = segment:sub(newline + 1)
    else
      break
    end
  end
  local leading = segment:match("^%s*(.*)$") or ""
  local attribute, value = leading:match("^([%a_][%w_]*)%s*=%s*(.*)$")
  return {
    attribute = attribute,
    value = value,
    shorthand = attribute == nil and leading,
    after_separator = after_separator,
    quote = quote,
    char_literal = char_literal,
    value_has_string = value_has_string,
    value_has_outside_text = value_has_outside_text,
    paren_depth = paren_depth,
    brace_depth = brace_depth,
  }
end

local function attribute_prefix_context(annotation, segment, after_separator)
  if segment == "" then
    return annotation ~= "Mixin"
  end
  if not after_separator and (annotation == "Mixin" or annotation == "At"
    or annotation == "Accessor" or annotation == "Invoker" or annotation == "Definition"
    or annotation == "Expression" or annotation == "Expressions" or annotation == "Share"
    or annotation == "Local" or annotation == "Wrap")
  then
    return false
  end
  if not segment:match("^[%a_][%w_]*$") then
    return false
  end
  local modes = annotation_attribute_modes[annotation]
  if not modes then return false end
  for name in pairs(modes) do
    if name:sub(1, #segment):lower() == segment:lower() then
      return true
    end
  end
  for name in pairs({
    value = true, targets = true, target = true, priority = true, remap = true,
    cancellable = true, locals = true, require = true, expect = true, allow = true,
    order = true, ordinal = true, index = true, opcode = true, shift = true,
    print = true, argsOnly = true, type = true, ["local"] = true, constant = true,
    nullValue = true, intValue = true, floatValue = true, doubleValue = true,
    longValue = true, classValue = true, log = true, name = true,
  }) do
    if name:sub(1, #segment):lower() == segment:lower() then
      return true
    end
  end
  return false
end

local function exclusive_string_value(segment)
  local value = (segment.value or segment.shorthand or ""):match("^%s*(.*)$") or ""
  if value == "" then
    return true
  end
  if segment.quote == '"' then
    return true
  end
  if segment.value_has_outside_text then
    return false
  end
  if segment.value_has_string then
    return true
  end
  return value:sub(1, 1) == "{"
end

function M.completion_context_at(bufnr, position)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  if M.detect_file_type(bufnr) ~= nil then
    return { owned = true, exclusive = true, kind = "file" }
  end

  if vim.bo[bufnr].filetype == "json" then
    if M.is_mcdev_completion_context(bufnr) then
      return { owned = true, exclusive = false, kind = "json" }
    end
    return nil
  end
  if vim.bo[bufnr].filetype ~= "java" then
    return nil
  end

  local source = buffer_text(bufnr)
  local imports = explicit_annotation_imports(source)
  local text = text_before_cursor(bufnr, position)
  local scan = scan_open_annotations(text, imports)
  if scan.line_comment or scan.block_comment or scan.char_literal then
    return nil
  end
  if scan.annotation_prefix then
    return {
      owned = true,
      exclusive = false,
      kind = "annotation_name",
      annotation = simple_annotation_name(scan.annotation_prefix),
    }
  end

  local frame = scan.frames[#scan.frames]
  if not frame then
    return member_header_context(text, text_after_cursor(bufnr, position), scan)
  end

  local segment = current_annotation_segment(text, frame)
  if name_only_annotations[frame.name] then
    return {
      owned = true,
      exclusive = false,
      kind = "java_value",
      annotation = frame.name,
      attribute = segment.attribute,
    }
  end
  local modes = annotation_attribute_modes[frame.name] or {}
  local mode = segment.attribute and modes[segment.attribute] or nil
  if mode == "exclusive" then
    if exclusive_string_value(segment) then
      return { owned = true, exclusive = true, kind = "value", annotation = frame.name, attribute = segment.attribute }
    end
    return { owned = true, exclusive = false, kind = "java_value", annotation = frame.name, attribute = segment.attribute }
  end
  if segment.attribute then
    return { owned = true, exclusive = false, kind = mode or "java_value", annotation = frame.name, attribute = segment.attribute }
  end

  local shorthand = segment.shorthand or ""
  if attribute_prefix_context(frame.name, shorthand, segment.after_separator) then
    return { owned = true, exclusive = true, kind = "attribute", annotation = frame.name }
  end
  if frame.name == "Mixin" then
    return { owned = true, exclusive = false, kind = "class", annotation = frame.name }
  end
  if shorthand == "" then
    return { owned = true, exclusive = frame.name ~= "Mixin", kind = "value", annotation = frame.name }
  end
  if shorthand:sub(1, 1) == '"' and (frame.name == "At" or frame.name == "Accessor" or frame.name == "Invoker"
    or frame.name == "Definition" or frame.name == "Expression" or frame.name == "Expressions"
    or frame.name == "Share" or frame.name == "Local" or frame.name == "Wrap")
  then
    if exclusive_string_value(segment) then
      return { owned = true, exclusive = true, kind = "value", annotation = frame.name }
    end
    return { owned = true, exclusive = false, kind = "java_value", annotation = frame.name }
  end
  return { owned = true, exclusive = false, kind = "java_value", annotation = frame.name }
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
    local imports = explicit_annotation_imports(text)
    for annotation in pairs(mcdev_annotation_names) do
      local imported = imports[annotation]
      if (not imported or mcdev_annotation_fqns[imported])
        and text:find("@" .. annotation, 1, true) ~= nil
      then
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
  return M.completion_context_at(bufnr, position) ~= nil
end

return M
