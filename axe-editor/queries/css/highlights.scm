; Selectors
(tag_name) @tag
(class_name) @property
(id_name) @property

; Properties
(property_name) @property

; Values
(plain_value) @variable
(color_value) @number
(integer_value) @number
(float_value) @number
(string_value) @string

; Keywords
(important) @keyword

; Comments
(comment) @comment

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ ":" ";" "," "." ] @punctuation.delimiter

; Operators
[ ">" "~" "+" ] @operator
