; Keywords
[
  "async"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "default"
  "delete"
  "do"
  "else"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "function"
  "get"
  "if"
  "import"
  "in"
  "instanceof"
  "let"
  "new"
  "of"
  "return"
  "set"
  "static"
  "switch"
  "throw"
  "try"
  "typeof"
  "var"
  "void"
  "while"
  "with"
  "yield"
] @keyword

; Functions
(function_declaration name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (member_expression property: (property_identifier) @function))
(method_definition name: (property_identifier) @function)
(arrow_function)

; Types / classes
(class_declaration name: (identifier) @type)

; Strings
(string) @string
(template_string) @string
(template_substitution) @string.special

; Comments
(comment) @comment

; Numbers
(number) @number

; Constants
[
  (true)
  (false)
  (null)
  (undefined)
] @constant.builtin

; Variables
(identifier) @variable
(property_identifier) @property
(shorthand_property_identifier) @property

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "**"
  "="
  "=="
  "==="
  "!="
  "!=="
  "<"
  ">"
  "<="
  ">="
  "&&"
  "||"
  "!"
  "??"
  "=>"
  "+="
  "-="
  "*="
  "/="
  "..."
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter

; Regex
(regex) @string.special

; this/super
(this) @variable.builtin
(super) @variable.builtin
