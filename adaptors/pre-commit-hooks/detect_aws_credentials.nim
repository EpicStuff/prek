import std/[json, re, strutils]

let findingRe = re"^AWS secret found in (.+): ([^ ]+)$"

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
        "ruleId": "detect-aws-credentials/aws-secret",
        "level": "error",
        "message": {"text": "AWS secret found: " & matches[1]},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[0]}}}]
      })

  let sarif = %*{
    "version": "2.1.0",
    "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
    "runs": [{"tool": {"driver": {"name": "detect-aws-credentials"}}, "results": results}]
  }

  stdout.write($sarif)
  stdout.write("\n")

main()
