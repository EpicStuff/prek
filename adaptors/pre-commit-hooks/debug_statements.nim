import std/[json, strutils]
import ./[tinyre, utils]

let parseFailRe = re"^(.+) - Could not parse ast$"
let debugStmtRe = re"^(.+):([0-9]+):([0-9]+): ([^ ]+ .+)$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var parseFailMatch: array[2, string]
    if line.match(parseFailRe, parseFailMatch) == 2:
      results.add(%*{
        "ruleId": "debug-statements/parse-error",
        "level": "error",
        "message": {"text": "Could not parse ast"},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": parseFailMatch[1]}}}]
      })
      continue

    var matches: array[5, string]
    if line.match(debugStmtRe, matches) == 5:
      try:
        let lineNum = parseInt(matches[2])
        let colNum = parseInt(matches[3]) + 1
        let message = matches[4]
        let ruleToken = message.split(maxsplit = 1)[0]

        results.add(%*{
          "ruleId": "debug-statements/" & ruleToken,
          "level": "warning",
          "message": {"text": message},
          "locations": [{
            "physicalLocation": {
              "artifactLocation": {"uri": matches[1]},
              "region": {"startLine": lineNum, "startColumn": colNum}
            }
          }]
        })
      except ValueError:
        discard

  writeSarif("debug-statements", results)

main()
