import std/[json, strutils]
import ./utils

const suffixPrefix = " does not match pattern \""

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    let sep = line.find(suffixPrefix)
    if sep <= 0 or not line.endsWith("\""):
      continue

    let filePath = line[0 ..< sep]
    let pattern = line[(sep + suffixPrefix.len) .. ^2]

    results.add(%*{
      "ruleId": "name-tests-test/pattern",
      "level": "warning",
      "message": {"text": "does not match pattern \"" & pattern & "\""},
      "locations": [{"physicalLocation": {"artifactLocation": {"uri": filePath}}}]
    })

  writeSarif("name-tests-test", results)

main()
