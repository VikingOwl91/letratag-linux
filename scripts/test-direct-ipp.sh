#!/usr/bin/env bash
set -euo pipefail
umask 077
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export PKG_CONFIG_PATH="$root/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cargo build --locked -p letratag-printer --features pappl
outroot=${LETRATAG_TEST_OUTPUT_ROOT:-"$root/test-output"}
mkdir -p "$outroot"
out=$(mktemp -d "$outroot/direct-ipp.XXXXXXXX")
completed_count() {
    local count
    count=$(find "$1" -name '*.state' -exec grep -l '^completed$' {} + | wc -l) || count=0
    printf '%s\n' "$count"
}
mkdir "$out/poll-state"
printf 'inflight\n' > "$out/poll-state/fixture.state"
count=$(completed_count "$out/poll-state")
((count==0))
printf 'completed\n' > "$out/poll-state/fixture.state"
count=$(completed_count "$out/poll-state")
((count==1))
port=${LETRATAG_TEST_PORT:-38681}
[[ $port =~ ^[0-9]+$ ]] && ((port>=1024&&port<=65534))
if ss -H -ltn "sport = :$port" | grep -q .; then echo 'Test port is occupied' >&2; exit 1; fi
rustc --edition=2024 tests/make-pdf.rs -o "$out/make-pdf"
rustc --edition=2024 tests/inspect-direct-captures.rs -o "$out/inspect"
rustc --edition=2024 --test tests/inspect-direct-captures.rs -o "$out/inspect-tests"
"$out/inspect-tests" > "$out/oracle-tests.log"
"$out/make-pdf" > "$out/label-50.pdf"
"$out/make-pdf" 180 > "$out/label-63.5.pdf"
binary=${CARGO_TARGET_DIR:-"$root/target"}/debug/letratag-printer
"$binary" serve --transport mock --profile legacy --capture --port "$port" \
    --state "$out/state" --spool "$out/spool" > "$out/server.log" 2>&1 &
server=$!
cleanup() { kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; }
trap cleanup EXIT
uri="ipp://127.0.0.1:$port/ipp/print"
ready=false
for _ in {1..50}; do
    if ipptool -T 2 "$uri" tests/ipp/get-printer.test > "$out/readiness.log" 2>&1; then ready=true; break; fi
    kill -0 "$server" 2>/dev/null || { cat "$out/server.log"; exit 1; }
    sleep 0.1
done
$ready || { cat "$out/server.log"; exit 1; }
ipptool -tv -T 5 "$uri" tests/ipp/get-printer.test > "$out/capabilities.log"
ipptool -tv -T 20 -d length=5000 -f "$out/label-50.pdf" "$uri" examples/print-pdf.test > "$out/explicit50.log"
ipptool -tv -T 20 -d length=6350 -f "$out/label-63.5.pdf" "$uri" examples/print-pdf.test > "$out/explicit63.5.log"
lp -h "127.0.0.1:$port" -d letratag -o media=custom_default_12x50mm \
    -o print-scaling=none "$out/label-50.pdf" > "$out/lp.log"
lpr -H "127.0.0.1:$port" -P letratag -o media=custom_default_12x50mm \
    -o print-scaling=none "$out/label-50.pdf"
for _ in {1..100}; do
    count=$(completed_count "$out/state")
    if ((count==4)); then break; fi
    sleep 0.1
done
[[ $count == 4 ]] || { cat "$out/server.log"; echo "Expected 4 completed mock jobs, got $count" >&2; exit 1; }
for log in "$out/explicit50.log" "$out/explicit63.5.log"; do
    jobid=$(sed -n 's/^ *job-id (integer) = \([0-9]*\)$/\1/p' "$log")
    [[ $jobid =~ ^[0-9]+$ ]]
    ipptool -tv -T 5 -d "jobid=$jobid" "$uri" examples/get-job.test > "$log.state"
    grep -q 'job-state (enum) = completed' "$log.state"
done
# A mismatched explicit media length must reject without another transport call.
ipptool -tv -T 20 -d length=6350 -f "$out/label-50.pdf" "$uri" \
    tests/ipp/print-and-expect-aborted.test > "$out/reject-length.log"
[[ $(find "$out/state" -name '*.state' | wc -l) == 4 ]]
"$out/inspect" "$out/state" > "$out/decoded.log"
cat "$out/decoded.log"
printf 'PASS: documented Direct IPP clients, explicit media, state queries and mismatch rejection. Evidence: %s\n' "$out"
