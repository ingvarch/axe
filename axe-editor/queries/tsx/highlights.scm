; Catch-all identifiers (lowest priority — specific patterns below override these)
(identifier) @variable
(property_identifier) @property

; Keywords
[
  "abstract"
  "async"
  "await"
  "break"
  "case"
  "catch"
  "class"
  "const"
  "continue"
  "debugger"
  "declare"
  "default"
  "delete"
  "do"
  "else"
  "enum"
  "export"
  "extends"
  "finally"
  "for"
  "from"
  "function"
  "get"
  "if"
  "implements"
  "import"
  "in"
  "instanceof"
  "interface"
  "keyof"
  "let"
  "namespace"
  "new"
  "of"
  "override"
  "readonly"
  "return"
  "satisfies"
  "set"
  "static"
  "switch"
  "throw"
  "try"
  "type"
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

; Types
(type_identifier) @type
(predefined_type) @type.builtin
(class_declaration name: (type_identifier) @type)
(interface_declaration name: (type_identifier) @type)
(enum_declaration name: (identifier) @type)

; JSX
(jsx_opening_element name: (identifier) @tag)
(jsx_closing_element name: (identifier) @tag)
(jsx_self_closing_element name: (identifier) @tag)
(jsx_attribute (property_identifier) @property)

; Strings
(string) @string
(template_string) @string

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

; Variables (catch-alls moved to top of file)

; Operators
[
  "+"
  "-"
  "*"
  "/"
  "%"
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
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter

(this) @variable.builtin
(super) @variable.builtin
