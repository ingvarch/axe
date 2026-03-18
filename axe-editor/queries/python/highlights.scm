; Catch-all identifiers (lowest priority — specific patterns below override these)
(identifier) @variable

; Keywords
[
  "and"
  "as"
  "assert"
  "async"
  "await"
  "break"
  "class"
  "continue"
  "def"
  "del"
  "elif"
  "else"
  "except"
  "exec"
  "finally"
  "for"
  "from"
  "global"
  "if"
  "import"
  "in"
  "is"
  "lambda"
  "nonlocal"
  "not"
  "or"
  "pass"
  "print"
  "raise"
  "return"
  "try"
  "while"
  "with"
  "yield"
  "match"
  "case"
] @keyword

; Functions
(function_definition name: (identifier) @function)
(call function: (identifier) @function)
(call function: (attribute attribute: (identifier) @function))

; Types / classes
(class_definition name: (identifier) @type)

; Strings
(string) @string
(interpolation) @string.special

; Comments
(comment) @comment

; Numbers
(integer) @number
(float) @number

; Constants
[
  (true)
  (false)
  (none)
] @constant.builtin

; Properties
(attribute attribute: (identifier) @property)

; Parameters
(parameters (identifier) @variable.parameter)

; Decorators
(decorator) @attribute

; Operators
[
  "+"
  "-"
  "*"
  "**"
  "/"
  "//"
  "%"
  "="
  "=="
  "!="
  "<"
  ">"
  "<="
  ">="
  "+="
  "-="
  "*="
  "/="
] @operator

; Punctuation
[ "(" ")" "{" "}" "[" "]" ] @punctuation.bracket
[ "," ";" ":" "." ] @punctuation.delimiter

; Builtins
((identifier) @function.builtin
  (#match? @function.builtin "^(abs|all|any|bin|bool|bytearray|bytes|callable|chr|classmethod|compile|complex|delattr|dict|dir|divmod|enumerate|eval|exec|filter|float|format|frozenset|getattr|globals|hasattr|hash|help|hex|id|input|int|isinstance|issubclass|iter|len|list|locals|map|max|memoryview|min|next|object|oct|open|ord|pow|print|property|range|repr|reversed|round|set|setattr|slice|sorted|staticmethod|str|sum|super|tuple|type|vars|zip)$"))
