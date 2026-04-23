import std/[json, strutils]
import ./utils

const parseMarker = ": failed parsing with "

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0 or not line.endsWith(":"):
      continue

    let markerIndex = line.find(parseMarker)
    if markerIndex <= 0:
      continue

    let filePath = line[0 ..< markerIndex]
    let parserInfo = line[(markerIndex + parseMarker.len) .. ^2]

    results.add(%*{
      "ruleId": "check-ast/syntax-error",
      "level": "error",
      "message": {"text": "failed parsing with " & parserInfo},
      "locations": [{
        "physicalLocation": {
          "artifactLocation": {"uri": filePath}
        }
      }]
    })

  writeSarif("check-ast", results)

main()
