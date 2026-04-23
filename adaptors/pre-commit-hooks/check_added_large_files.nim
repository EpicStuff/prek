import std/[json, strutils, os]
import tinyre
import utils


proc main(input: string, ruleId = "pre-commit/check-case-conflict"): JsonNode =
  var results = newJArray()

  for line in input.splitLines():
    if line.strip().len == 0:
      continue

    let line2 = line.match(re"^\s*(.+) \(([^)]+)\) exceeds .+$")
    if line2.len != 3 or not line2[1].strip().fileExists():
      stderr.writeLine("Warning: failed to parse line: " & line)
    else:
      results.add locationResult(ruleId, line.strip(), line2[1].strip())

  return buildSarif(
    ruleId,
    "pre-commit check-case-conflict",
    "Detects files that would collide on case-insensitive filesystems",
    results
  )

when isMainModule:
  echo main(stdin.readAll())
