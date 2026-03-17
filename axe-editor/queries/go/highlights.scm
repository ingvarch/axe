; Keywords
[
  "break"
  "case"
  "chan"
  "const"
  "continue"
  "default"
  "defer"
  "else"
  "fallthrough"
  "for"
  "func"
  "go"
  "goto"
  "if"
  "import"
  "interface"
  "map"
  "package"
  "range"
  "return"
  "select"
  "struct"
  "switch"
  "type"
  "var"
] @keyword

; Functions
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (selector_expression field: (field_identifier) @function))
(method_declaration name: (field_identifier) @function)

; Types
(type_identifier) @type
(type_spec name: (type_identifier) @type)

; Strings
(raw_string_literal) @string
(interpreted_string_literal) @string
(rune_literal) @string

; Comments
(comment) @comment

; Numbers
(int_literal) @number
(float_literal) @number
(imaginary_literal) @number

; Constants
[
  (true)
  (false)
  (nil)
  (iota)
] @constant.builtin

; Variables
(identifier) @variable
(field_identifier) @property

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
  ":="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "&"
  "|"
  "^"
  "<<"
  ">>"
  "<-"
  "+="
  "-="
  "*="
  "/="
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter

; Package names
(package_clause (package_identifier) @variable)
