import std/[json, strutils]
import ./[tinyre, utils]

let parseFailRe = re"^(.+): failed parsing with (.+):$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[3, string]
    if line.match(parseFailRe, matches) == 3:
      results.add(%*{
        "ruleId": "check-ast/syntax-error",
        "level": "error",
        "message": {"text": "failed parsing with " & matches[2]},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[1]}}}]
      })

  writeSarif("check-ast", results)

main()
