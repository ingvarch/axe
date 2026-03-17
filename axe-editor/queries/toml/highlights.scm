; Keys
(bare_key) @property
(dotted_key (bare_key) @property)
(quoted_key) @property

; Strings
(string) @string

; Numbers
(integer) @number
(float) @number

; Booleans
(boolean) @constant.builtin

; Dates
(local_date) @string.special
(local_time) @string.special
(local_date_time) @string.special
(offset_date_time) @string.special

; Comments
(comment) @comment

; Tables
(table (bare_key) @type)
(table (dotted_key (bare_key) @type))
(table (quoted_key) @type)
(table_array_element (bare_key) @type)
(table_array_element (dotted_key (bare_key) @type))
(table_array_element (quoted_key) @type)

; Punctuation
[ "[" "]" "[[" "]]" ] @punctuation.bracket
[ "." "," ] @punctuation.delimiter
[ "=" ] @operator
