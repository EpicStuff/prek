import std/[json, strutils]
import tinyre
import ./utils

let findingRe = re"""^(.+) does not match pattern "(.+)"$"""

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    let matches = line.match(findingRe)
    if matches.len >= 3:
      results.add(%*{
        "ruleId": "name-tests-test/pattern",
        "level": "warning",
        "message": {"text": "does not match pattern \"" & matches[2] & "\""},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[1]}}}]
      })
  writeSarif("name-tests-test", results)

main()
