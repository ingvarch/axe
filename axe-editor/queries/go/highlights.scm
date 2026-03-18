; Catch-all identifiers (lowest priority — specific patterns below override these)
(identifier) @variable
(field_identifier) @property
(package_identifier) @variable

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

; Built-in functions
((identifier) @function.builtin
  (#match? @function.builtin "^(append|cap|close|complex|copy|delete|imag|len|make|new|panic|print|println|real|recover)$"))

; Types
(type_identifier) @type
(type_spec name: (type_identifier) @type)

; Built-in types
((type_identifier) @type.builtin
  (#match? @type.builtin "^(bool|byte|complex64|complex128|error|float32|float64|int|int8|int16|int32|int64|rune|string|uint|uint8|uint16|uint32|uint64|uintptr|any|comparable)$"))

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

; Package clause
(package_clause (package_identifier) @variable)
