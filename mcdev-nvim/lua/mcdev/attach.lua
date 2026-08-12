local config = require("mcdev.config")
local navigation = require("mcdev.navigation")
local code_action = require("mcdev.code_action")
local hover = require("mcdev.hover")
local lsp = require("mcdev.lsp")

local M = {}

local function cursor_range()
  local position = vim.api.nvim_win_get_cursor(0)
  return {
    start = { line = position[1] - 1, character = position[2] },
    ["end"] = { line = position[1] - 1, character = position[2] },
  }
end

local function visual_range(bufnr)
  local start_pos = vim.api.nvim_buf_get_mark(bufnr, "<")
  local end_pos = vim.api.nvim_buf_get_mark(bufnr, ">")
  return {
    start = { line = start_pos[1] - 1, character = start_pos[2] },
    ["end"] = { line = end_pos[1] - 1, character = end_pos[2] + 1 },
  }
end

local function select_code_action(actions, bufnr)
  if #actions == 1 then
    code_action.apply(actions[1], bufnr)
    return
  end
  vim.ui.select(actions, {
    prompt = "mcdev code action:",
    format_item = function(action)
      return action.title or action.command or "(untitled action)"
    end,
  }, function(choice)
    if choice then
      code_action.apply(choice, bufnr)
    end
  end)
end

local function request_code_actions(bufnr, range)
  local diagnostic_codes = vim.tbl_map(function(diagnostic)
    return diagnostic.code
  end, vim.diagnostic.get(bufnr, { namespace = require("mcdev.diagnostics").namespace }))
  local provider = config.options.standard_lsp.prefer and lsp.code_actions or code_action.code_actions
  provider(bufnr, range, diagnostic_codes, function(actions, err)
    if err then
      vim.notify(tostring(err), vim.log.levels.WARN)
      return
    end
    if not actions or #actions == 0 then
      vim.notify("mcdev: no code actions available", vim.log.levels.INFO)
      return
    end
    select_code_action(actions, bufnr)
  end)
end

local function goto_location(locations, err, label, raw_locations)
  if err then
    vim.notify(tostring(err), vim.log.levels.WARN)
    return
  end
  if locations and #locations > 0 then
    if #locations == 1 then
      vim.lsp.util.show_document(locations[1], "utf-8", { focus = true })
      return
    end
    vim.ui.select(locations, {
      prompt = "mcdev " .. label .. ":",
      format_item = function(location)
        return location.uri
      end,
    }, function(choice)
      if choice then
        vim.lsp.util.show_document(choice, "utf-8", { focus = true })
      end
    end)
    return
  end
  if raw_locations and #raw_locations > 0 then
    local message = raw_locations[1].resolutionMessage or "definition target has no navigable source"
    vim.notify("mcdev: " .. message, vim.log.levels.WARN)
    return
  end
  vim.notify("mcdev: no " .. label .. " found", vim.log.levels.INFO)
end

function M.setup(bufnr)
  bufnr = bufnr or 0
  local completion_opts = config.options.completion or {}
  if completion_opts.omnifunc ~= false then
    vim.bo[bufnr].omnifunc = "v:lua.require'mcdev.omnifunc'.complete"
  end

  local nav_opts = config.options.navigation or {}
  if nav_opts.enable then
    vim.keymap.set("n", "gd", function()
      local provider = config.options.standard_lsp.prefer and lsp.definition or navigation.definition
      provider(bufnr, nil, function(locations, err, raw_locations)
        goto_location(locations, err, "definition", raw_locations)
      end)
    end, { buffer = bufnr, desc = "Mcdev go to definition" })

    vim.keymap.set("n", "gr", function()
      local provider = config.options.standard_lsp.prefer and lsp.references or navigation.references
      provider(bufnr, nil, function(locations, err)
        goto_location(locations, err, "references")
      end)
    end, { buffer = bufnr, desc = "Mcdev find references" })

    vim.keymap.set("n", "K", function()
      if config.options.standard_lsp.prefer then
        lsp.hover(bufnr, nil, function(result, err)
          if err then
            vim.notify(tostring(err), vim.log.levels.WARN)
            return
          end
          local contents = result and (result.contents or result)
          if contents and contents.value then contents = { contents.value } end
          if contents and #contents > 0 then
            vim.lsp.util.open_floating_preview(contents, "markdown", { border = "rounded" })
          else
            hover.show(bufnr)
          end
        end)
      else
        hover.show(bufnr)
      end
    end, { buffer = bufnr, desc = "Mcdev hover" })
  end

  local code_action_opts = config.options.code_action or {}
  if code_action_opts.enable then
    vim.keymap.set("n", "<leader>ca", function()
      request_code_actions(bufnr, cursor_range())
    end, { buffer = bufnr, desc = "Mcdev code action" })
    vim.keymap.set("v", "<leader>ca", function()
      request_code_actions(bufnr, visual_range(bufnr))
    end, { buffer = bufnr, desc = "Mcdev code action" })
  end
end

return M
