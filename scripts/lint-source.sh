#!/usr/bin/env bash
set -euo pipefail

max_lines=400
status=0

while IFS= read -r file; do
  [[ -f $file ]] || continue
  lines=$(wc -l <"$file" | tr -d ' ')
  if ((lines > max_lines)); then
    echo "$file: $lines lines, the limit is $max_lines"
    status=1
  fi
  if ! awk -v file="$file" '
    /(^|[[:space:]])\/[\/*]/ && !/\/\/ SAFETY: / {
      print file ":" NR ": comment; only `// SAFETY:` is allowed"
      found = 1
    }
    END { exit found }
  ' "$file"; then
    status=1
  fi
done < <(git ls-files --cached --others --exclude-standard -- 'crates/*.rs')

exit $status
