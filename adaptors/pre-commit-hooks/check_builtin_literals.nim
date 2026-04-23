import std/[json, strutils]
import ./utils

const marker = ": replace "

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    let markerIndex = line.find(marker)
    if markerIndex <= 0:
      continue

    let loc = line[0 ..< markerIndex]
    let messagePart = line[(markerIndex + 2) .. ^1]

    let locParts = loc.rsplit(':', maxsplit = 2)
    if locParts.len != 3:
      continue

    try:
      let filePath = locParts[0]
      let lineNum = parseInt(locParts[1])
      let colNum = parseInt(locParts[2]) + 1

      if not messagePart.startsWith("replace "):
        continue
      let replacementText = messagePart[8 .. ^1]
      let callName = replacementText.split("()", maxsplit = 1)[0]

      results.add(%*{
        "ruleId": "check-builtin-literals/" & callName,
        "level": "warning",
        "message": {"text": messagePart},
        "locations": [{
          "physicalLocation": {
            "artifactLocation": {"uri": filePath},
            "region": {"startLine": lineNum, "startColumn": colNum}
          }
        }]
      })
    except ValueError:
      discard

  writeSarif("check-builtin-literals", results)

main()
