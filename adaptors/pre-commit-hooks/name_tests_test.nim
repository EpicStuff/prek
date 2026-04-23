import std/[json, strutils]
import ./[tinyre, utils]

let findingRe = re"""^(.+) does not match pattern "(.+)"$"""

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[3, string]
    if line.match(findingRe, matches) == 3:
      results.add(%*{
        "ruleId": "name-tests-test/pattern",
        "level": "warning",
        "message": {"text": "does not match pattern \"" & matches[2] & "\""},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[1]}}}]
      })

  writeSarif("name-tests-test", results)

main()
