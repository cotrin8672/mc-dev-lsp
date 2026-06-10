local function assert_eq(actual, expected, message)
  if actual ~= expected then
    error((message or "assertion failed") .. ": expected " .. vim.inspect(expected) .. ", got " .. vim.inspect(actual))
  end
end

local function assert_true(value, message)
  if not value then
    error(message or "expected true")
  end
end

local function assert_not_nil(value, message)
  if value == nil then
    error(message or "expected non-nil value")
  end
end

local function assert_nil(value, message)
  if value ~= nil then
    error((message or "expected nil") .. ": got " .. vim.inspect(value))
  end
end

return {
  assert_eq = assert_eq,
  assert_true = assert_true,
  assert_not_nil = assert_not_nil,
  assert_nil = assert_nil,
}
