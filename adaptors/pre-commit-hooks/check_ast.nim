import std/[json, re, strutils]
import ./utils

let parseFailRe = re"^(.+): failed parsing with (.+):$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[2, string]
    if line.match(parseFailRe, matches):
      results.add(%*{
        "ruleId": "check-ast/syntax-error",
        "level": "error",
        "message": {"text": "failed parsing with " & matches[1]},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": matches[0]}
          }
        }]
      })
  writeSarif("check-ast", results)

main()
