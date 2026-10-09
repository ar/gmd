-- Email bodies accept Markdown, not executable HTML or local/remote image loads.
function CodeBlock(block)
  -- 'shell' is a common Markdown fence label but not a Pandoc syntax name.
  if block.classes[1] and block.classes[1]:lower() == "shell" then
    block.classes[1] = "bash"
  end
  return block
end

function RawBlock(block)
  return pandoc.CodeBlock(block.text)
end

function RawInline(inline)
  return pandoc.Code(inline.text)
end

function Link(link)
  local target = link.target:lower():gsub("%c", ""):gsub("%s", "")
  if target:match("^https?://") or target:match("^mailto:") then
    return link
  end
  return link.content
end

function Image(image)
  local content = pandoc.List({pandoc.Str("[Image: ")})
  content:extend(image.caption)
  content:insert(pandoc.Str("]"))
  return pandoc.Span(content)
end
