import std/[json, re, strutils]

let findingRe = re"""^(.+) does not match pattern "(.+)"$"""

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0:
      continue

    var matches: array[2, string]
    if line.match(findingRe, matches):
      results.add(%*{
        "ruleId": "name-tests-test/pattern",
        "level": "warning",
        "message": {"text": "does not match pattern \"" & matches[1] & "\""},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[0]}}}]
      })

  let sarif = %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [{"tool": {"driver": {"name": "name-tests-test"}}, "results": results}]
  }

  stdout.write($sarif)
  stdout.write("\n")

main()
