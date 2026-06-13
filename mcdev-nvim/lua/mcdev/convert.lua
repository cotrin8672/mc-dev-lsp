local M = {}

local severity_map = {
  error = vim.diagnostic.severity.ERROR,
  warning = vim.diagnostic.severity.WARN,
  info = vim.diagnostic.severity.INFO,
  hint = vim.diagnostic.severity.HINT,
}

function M.unwrap_envelope(envelope, err)
  if err then
    return nil, err
  end
  if not envelope then
    return nil, "mcdev: empty response"
  end
  if envelope.error then
    return nil, envelope.error.message or vim.inspect(envelope.error)
  end
  return envelope.result
end

function M.to_lsp_location(location, fallback_uri)
  local uri = location.documentUri
  if (uri == nil or uri == "") and fallback_uri then
    uri = fallback_uri
  end
  return {
    uri = uri,
    resolution = location.resolution,
    resolutionMessage = location.resolutionMessage,
    range = {
      start = {
        line = location.range.start.line,
        character = location.range.start.character,
      },
      ["end"] = {
        line = location.range["end"].line,
        character = location.range["end"].character,
      },
    },
  }
end

function M.to_vim_diagnostic(diagnostic)
  return {
    lnum = diagnostic.range.start.line,
    col = diagnostic.range.start.character,
    end_lnum = diagnostic.range["end"].line,
    end_col = diagnostic.range["end"].character,
    severity = severity_map[diagnostic.severity] or vim.diagnostic.severity.ERROR,
    message = diagnostic.message,
    code = diagnostic.code,
    source = "mcdev",
    user_data = diagnostic.metadata,
  }
end

local function to_lsp_text_edit(text_edit)
  return {
    range = {
      start = {
        line = text_edit.range.start.line,
        character = text_edit.range.start.character,
      },
      ["end"] = {
        line = text_edit.range["end"].line,
        character = text_edit.range["end"].character,
      },
    },
    newText = text_edit.newText,
  }
end

function M.to_lsp_code_action(action)
  local document_changes = {}
  for _, workspace_edit in ipairs(action.edits or {}) do
    table.insert(document_changes, {
      textDocument = {
        uri = workspace_edit.documentUri,
        version = nil,
      },
      edits = vim.tbl_map(to_lsp_text_edit, workspace_edit.edits or {}),
    })
  end
  return {
    title = action.title,
    kind = action.kind,
    edit = {
      documentChanges = document_changes,
    },
    data = action.metadata,
  }
end

return M
