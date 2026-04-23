import std/[json, strutils]
import ./utils

const prefix = "AWS secret found in "

proc main() =
  let input = stdin.readAll()
  var results = newJArray()

  for rawLine in input.splitLines():
    let line = rawLine.strip()
    if line.len == 0 or not line.startsWith(prefix):
      continue

    let remainder = line[prefix.len .. ^1]
    let sep = remainder.find(": ")
    if sep <= 0:
      continue

    let filePath = remainder[0 ..< sep]
    let key = remainder[(sep + 2) .. ^1]

    results.add(%*{
      "ruleId": "detect-aws-credentials/aws-secret",
      "level": "error",
      "message": {"text": "AWS secret found: " & key},
      "locations": [{"physicalLocation": {"artifactLocation": {"uri": filePath}}}]
    })

  writeSarif("detect-aws-credentials", results)

main()
