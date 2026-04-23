import std/[json, strutils]
import tinyre
import ./utils

let debugStmtRe = re"^(.+):([0-9]+):([0-9]+): ([^ ]+) (imported|called)$"
let parseFailRe = re"^(.+) - Could not parse ast$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    let parseMatches = line.match(parseFailRe)
    if parseMatches.len >= 2:
      results.add(%*{
        "ruleId": "debug-statements/parse-error",
        "level": "error",
        "message": {"text": "Could not parse ast"},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": parseMatches[1]}}}]
      })
      continue

    let matches = line.match(debugStmtRe)
    if matches.len >= 6:
      let lineNum = parseInt(matches[2])
      let colNum = parseInt(matches[3]) + 1
      results.add(%*{
        "ruleId": "debug-statements/" & matches[4],
        "level": "warning",
        "message": {"text": matches[4] & " " & matches[5]},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": matches[1]},
            "region": {"startLine": lineNum, "startColumn": colNum}
          }
        }]
      })
  writeSarif("debug-statements", results)

main()
