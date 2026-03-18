; Keywords
[
  "if"
  "else"
  "endif"
  "for"
  "endfor"
  "in"
] @keyword

; Comments
(comment) @comment

; Literals
(numeric_lit) @number
(bool_lit) @constant.builtin
(null_lit) @constant.builtin

; Strings and templates
[
  (quoted_template_start)
  (quoted_template_end)
  (template_literal)
] @string

[
  (heredoc_identifier)
  (heredoc_start)
] @string

; Template interpolation
[
  (template_interpolation_start)
  (template_interpolation_end)
  (template_directive_start)
  (template_directive_end)
  (strip_marker)
] @string.special

; Functions
(function_call (identifier) @function)

; Block types (resource, variable, data, locals, etc.)
(block (identifier) @type)

; Attributes
(attribute (identifier) @property)

; Object element keys
(object_elem key: (expression (variable_expr (identifier) @property)))

; Built-in type identifiers
((identifier) @type.builtin
  (#match? @type.builtin "^(bool|string|number|object|tuple|list|map|set|any)$"))

; Variables
(identifier) @variable

; Operators
[
  "!"
  "\*"
  "/"
  "%"
  "\+"
  "-"
  ">"
  ">="
  "<"
  "<="
  "=="
  "!="
  "&&"
  "||"
] @operator

[
  "="
  ":"
] @operator

; Punctuation
[
  "{"
  "}"
  "["
  "]"
  "("
  ")"
] @punctuation.bracket

[
  "."
  ".*"
  ","
  "[*]"
] @punctuation.delimiter

[
  (ellipsis)
  "\?"
  "=>"
] @punctuation.special
