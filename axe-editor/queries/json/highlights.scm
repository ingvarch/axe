; Properties
(pair key: (string) @property)

; Strings
(string) @string

; Numbers
(number) @number

; Constants
[
  (true)
  (false)
  (null)
] @constant.builtin

; Punctuation
[ "{" "}" "[" "]" ] @punctuation.bracket
[ "," ":" ] @punctuation.delimiter
