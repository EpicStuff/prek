import std/[json, strutils, os]
import utils


proc main(input: string, ruleId = "pre-commit/check-illegal-windows-names"): JsonNode =
  var results = newJArray()

  for line in input.splitLines():
    let line2 = line.strip()
    if line2.len == 0 or line2 == "Illegal Windows filenames detected":
      continue

    if line2.fileExists():
      results.add locationResult(ruleId, "Illegal Windows filenames detected", line2)
    else:
      stderr.writeLine("Warning: failed to parse line: " & line)

  return buildSarif(
    ruleId,
    "pre-commit check-illegal-windows-names",
    "Detects file names illegal on Windows",
    results
  )

when isMainModule:
  echo main(stdin.readAll())
