; Keywords
[
  "if"
  "then"
  "else"
  "elif"
  "fi"
  "case"
  "esac"
  "for"
  "do"
  "done"
  "while"
  "until"
  "in"
  "function"
  "select"
  "local"
  "declare"
  "export"
  "readonly"
  "unset"
] @keyword

; Functions
(function_definition name: (word) @function)
(command_name (word) @function)

; Strings
(string) @string
(raw_string) @string
(heredoc_body) @string

; Comments
(comment) @comment

; Numbers
(number) @number

; Variables
(variable_name) @variable
(special_variable_name) @variable.builtin
(simple_expansion (variable_name) @variable)

; Operators
[
  "="
  "=="
  "!="
  "<"
  ">"
  "&&"
  "||"
  "|"
  ">"
  ">>"
  "<"
  "<<"
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" "[[" "]]" ] @punctuation.bracket
[ ";" ] @punctuation.delimiter

; Expansion
"$" @punctuation.special
