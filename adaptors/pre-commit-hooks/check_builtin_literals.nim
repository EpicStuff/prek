import std/[json, strutils]
import ./[tinyre, utils]

let findingRe = re"^(.+):([0-9]+):([0-9]+): (replace .+)$"
let callNameRe = re"^replace ([A-Za-z_][A-Za-z0-9_]*)\(\) with .+$"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[5, string]
    if line.match(findingRe, matches) == 5:
      try:
        let lineNum = parseInt(matches[2])
        let colNum = parseInt(matches[3]) + 1
        let message = matches[4]

        var callNameMatch: array[2, string]
        if message.match(callNameRe, callNameMatch) != 2:
          continue

        results.add(%*{
          "ruleId": "check-builtin-literals/" & callNameMatch[1],
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

  writeSarif("check-builtin-literals", results)

main()
