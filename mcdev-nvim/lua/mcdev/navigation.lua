local convert = require("mcdev.convert")
local protocol = require("mcdev.protocol")

local M = {}

local function to_lsp_locations(result, fallback_uri)
  local locations = {}
  for _, location in ipairs((result and result.locations) or {}) do
    table.insert(locations, convert.to_lsp_location(location, fallback_uri))
  end
  return locations
end

local function filter_navigable(locations)
  return vim.tbl_filter(function(location)
    return location.uri ~= nil
      and location.uri ~= ""
      and location.resolution ~= "unresolved"
      and location.resolution ~= "bytecode_only"
  end, locations or {})
end

function M.definition(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  protocol.definition(bufnr, position, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    if cb then
      cb(
        filter_navigable(to_lsp_locations(result, vim.uri_from_bufnr(bufnr))),
        nil,
        result and result.locations or {}
      )
    end
  end)
end

function M.references(bufnr, position, cb)
  bufnr = bufnr or vim.api.nvim_get_current_buf()
  protocol.references(bufnr, position, function(envelope, err)
    local result, unwrap_err = convert.unwrap_envelope(envelope, err)
    if unwrap_err then
      if cb then
        cb(nil, unwrap_err)
      end
      return
    end
    if cb then
      cb(filter_navigable(to_lsp_locations(result)), nil)
    end
  end)
end

return M
