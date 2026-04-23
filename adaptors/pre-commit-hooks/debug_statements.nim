import std/[json, strutils]
import ./utils

const parseSuffix = " - Could not parse ast"

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    if line.endsWith(parseSuffix):
      let filePath = line[0 ..< (line.len - parseSuffix.len)]
      results.add(%*{
        "ruleId": "debug-statements/parse-error",
        "level": "error",
        "message": {"text": "Could not parse ast"},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": filePath}}}]
      })
      continue

    let messageSep = line.find(": ")
    if messageSep <= 0:
      continue

    let loc = line[0 ..< messageSep]
    let messagePart = line[(messageSep + 2) .. ^1]
    let locParts = loc.rsplit(':', maxsplit = 2)
    if locParts.len != 3:
      continue

    try:
      let filePath = locParts[0]
      let lineNum = parseInt(locParts[1])
      let colNum = parseInt(locParts[2]) + 1
      let messageTokens = messagePart.split(maxsplit = 1)
      if messageTokens.len == 0:
        continue

      results.add(%*{
        "ruleId": "debug-statements/" & messageTokens[0],
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

  writeSarif("debug-statements", results)

main()
