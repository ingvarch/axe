; Keywords
[
  "as"
  "async"
  "await"
  "break"
  "const"
  "continue"
  "dyn"
  "else"
  "enum"
  "extern"
  "fn"
  "for"
  "if"
  "impl"
  "in"
  "let"
  "loop"
  "match"
  "mod"
  "move"
  "pub"
  "ref"
  "return"
  "static"
  "struct"
  "trait"
  "type"
  "unsafe"
  "use"
  "where"
  "while"
  "yield"
] @keyword

; Named keyword nodes
(crate) @keyword
(self) @keyword
(super) @keyword
(mutable_specifier) @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin

; Functions
(function_item name: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(generic_function function: (identifier) @function)
(macro_invocation macro: (identifier) @function.macro)

; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

; Comments
(line_comment) @comment
(block_comment) @comment

; Numbers
(integer_literal) @number
(float_literal) @number

; Constants / booleans
(boolean_literal) @constant.builtin

; Variables
(identifier) @variable
(field_identifier) @property
(shorthand_field_initializer (identifier) @variable)

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
  "="
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
  "+="
  "-="
  "*="
  "/="
  "%="
  "=>"
  "->"
  ".."
  "..="
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" "::" ":" "." ] @punctuation.delimiter

; Attributes
(attribute_item) @attribute

; Escape sequences
(escape_sequence) @escape

; Lifetime
(lifetime (identifier) @variable.special)
