import std/[json, re, strutils]
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

    var parseMatches: array[1, string]
    if line.match(parseFailRe, parseMatches):
      results.add(%*{
        "ruleId": "debug-statements/parse-error",
        "level": "error",
        "message": {"text": "Could not parse ast"},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": parseMatches[0]}}}]
      })
      continue

    var matches: array[5, string]
    if line.match(debugStmtRe, matches):
      let lineNum = parseInt(matches[1])
      let colNum = parseInt(matches[2]) + 1
      results.add(%*{
        "ruleId": "debug-statements/" & matches[3],
        "level": "warning",
        "message": {"text": matches[3] & " " & matches[4]},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": matches[0]},
            "region": {"startLine": lineNum, "startColumn": colNum}
          }
        }]
      })
  writeSarif("debug-statements", results)

main()
