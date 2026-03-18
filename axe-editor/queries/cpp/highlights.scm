; Catch-all identifiers (lowest priority — specific patterns below override these)
(identifier) @variable
(field_identifier) @property

; Keywords
[
  "break"
  "case"
  "catch"
  "class"
  "co_await"
  "co_return"
  "co_yield"
  "concept"
  "const"
  "consteval"
  "constexpr"
  "constinit"
  "continue"
  "decltype"
  "default"
  "delete"
  "do"
  "else"
  "enum"
  "explicit"
  "extern"
  "final"
  "for"
  "friend"
  "goto"
  "if"
  "inline"
  "mutable"
  "namespace"
  "new"
  "noexcept"
  "operator"
  "override"
  "private"
  "protected"
  "public"
  "register"
  "requires"
  "return"
  "sizeof"
  "static"
  "static_assert"
  "struct"
  "switch"
  "template"
  "throw"
  "try"
  "typedef"
  "typename"
  "union"
  "using"
  "virtual"
  "volatile"
  "while"
] @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(auto) @type.builtin

; Functions
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))
(template_function name: (identifier) @function)

; Strings
(string_literal) @string
(raw_string_literal) @string
(char_literal) @string

; Comments
(comment) @comment

; Numbers
(number_literal) @number

; Constants
[
  (true)
  (false)
  (null)
] @constant.builtin

(this) @variable.builtin

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
  "~"
  "<<"
  ">>"
  "->"
  "::"
  ".*"
  "->*"
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" "<" ">" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter

; Preprocessor
"#define" @keyword
"#include" @keyword
"#if" @keyword
"#ifdef" @keyword
"#ifndef" @keyword
"#else" @keyword
"#endif" @keyword
(preproc_include) @keyword
