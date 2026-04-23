import std/[json, strutils]
import tinyre
import ./utils

let findingRe = re"^AWS secret found in (.+): ([^ ]+)$"

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
        "ruleId": "detect-aws-credentials/aws-secret",
        "level": "error",
        "message": {"text": "AWS secret found: " & matches[2]},
        "locations": [{"physicalLocation": {"artifactLocation": {"uri": matches[1]}}}]
      })
  writeSarif("detect-aws-credentials", results)

main()
