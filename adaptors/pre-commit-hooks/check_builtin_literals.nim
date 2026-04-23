import std/[json, strutils]
import tinyre
import ./utils

let findingRe = re"^(.+):([0-9]+):([0-9]+): replace ([a-zA-Z_][a-zA-Z0-9_]*)\(\) with (.+)$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    let matches = line.match(findingRe)
    if matches.len >= 6:
      let lineNum = parseInt(matches[2])
      let colNum = parseInt(matches[3]) + 1
      let replacement = matches[5]
      results.add(%*{
        "ruleId": "check-builtin-literals/" & matches[4],
        "level": "warning",
        "message": {"text": "replace " & matches[4] & "() with " & replacement},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": matches[1]},
            "region": {"startLine": lineNum, "startColumn": colNum}
          }
        }]
      })
  writeSarif("check-builtin-literals", results)

main()
