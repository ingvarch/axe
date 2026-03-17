; Keywords
[
  "break"
  "case"
  "const"
  "continue"
  "default"
  "do"
  "else"
  "enum"
  "extern"
  "for"
  "goto"
  "if"
  "inline"
  "register"
  "return"
  "sizeof"
  "static"
  "struct"
  "switch"
  "typedef"
  "union"
  "volatile"
  "while"
] @keyword

; Types
(type_identifier) @type
(primitive_type) @type.builtin
(sized_type_specifier) @type.builtin

; Functions
(function_declarator declarator: (identifier) @function)
(call_expression function: (identifier) @function)
(call_expression function: (field_expression field: (field_identifier) @function))

; Preprocessor
(preproc_directive) @keyword
(preproc_include) @keyword
"#define" @keyword
"#include" @keyword
"#if" @keyword
"#ifdef" @keyword
"#ifndef" @keyword
"#else" @keyword
"#endif" @keyword

; Strings
(string_literal) @string
(char_literal) @string
(system_lib_string) @string

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
  "+="
  "-="
  "*="
  "/="
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter
