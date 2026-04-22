import std/[json, re, strutils]
import ./utils

let findingRe = re"^(.+):([0-9]+):([0-9]+): replace ([a-zA-Z_][a-zA-Z0-9_]*)\(\) with (.+)$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[5, string]
    if line.match(findingRe, matches):
      let lineNum = parseInt(matches[1])
      let colNum = parseInt(matches[2]) + 1
      let replacement = matches[4]
      results.add(%*{
        "ruleId": "check-builtin-literals/" & matches[3],
        "level": "warning",
        "message": {"text": "replace " & matches[3] & "() with " & replacement},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": matches[0]},
            "region": {"startLine": lineNum, "startColumn": colNum}
          }
        }]
      })
  writeSarif("check-builtin-literals", results)

main()
