# How to Run a Goal-Driven Verification Loop

Combine an outer POSIX `until` loop, a deterministic verifier script, and `-c` WAL persistence so the Agent iterates until objective checks pass.

## Pattern
```sh
CTX="/tmp/goal-$(date +%s).ctx.jsonl"
MAX_ROUNDS=8
round=0

until ./verify.sh > /tmp/verify.log 2>&1; do
  round=$((round + 1))
  [ "$round" -gt "$MAX_ROUNDS" ] && { echo "Goal budget exceeded" >&2; exit 1; }
  ./coder.agent -c "$CTX" 4< /tmp/verify.log -- \
    "Round $round: fix all failures shown in fd 4 so ./verify.sh exits 0."
done
```

## Pitfalls
- Always bound iterations (`MAX_ROUNDS`) to avoid infinite token burn on unsolvable errors.
- Pass verifier output via `fd 4` (`4< /tmp/verify.log`) or `stdin` so each round stays compact.
- Use `aclr -t <turn_id> "<summary>"` inside `msh` to fold dead-end attempts while preserving prefix cache.
- Never trust self-reported success alone; rely on `./verify.sh` exit code `0`.
