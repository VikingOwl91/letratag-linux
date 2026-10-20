#!/usr/bin/env bash
set -euo pipefail
umask 077
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
export PKG_CONFIG_PATH="$root/.research/pappl-prefix/lib/pkgconfig${PKG_CONFIG_PATH:+:$PKG_CONFIG_PATH}"
cargo build --locked -p letratag-printer --features pappl
outroot=${LETRATAG_TEST_OUTPUT_ROOT:-"$root/test-output"}
mkdir -p "$outroot"
out=$(mktemp -d "$outroot/ipp.XXXXXXXX")
port=${LETRATAG_TEST_PORT:-38631}
[[ $port =~ ^[0-9]+$ ]] && ((port>=1024&&port<=65534))
if ss -H -ltn "sport = :$port" | grep -q .; then echo 'Test port is occupied' >&2;exit 1;fi
rustc --edition=2024 tests/make-pdf.rs -o "$out/make-pdf"
"$out/make-pdf" > "$out/label.pdf"
cc tests/make-raster.c -lcups -o "$out/make-raster"
"$out/make-raster" > "$out/label.pwg"
rustc --edition=2024 tests/ipp-client.rs -o "$out/ipp-client"
rustc --edition=2024 tests/inspect-captures.rs -o "$out/inspect-captures"
qpdf --empty --pages "$out/label.pdf" 1 "$out/label.pdf" 1 -- "$out/two-pages.pdf"
qpdf --check "$out/two-pages.pdf" > "$out/two-pages-check.log"
printf 'RaS2PwgR' > "$out/truncated-header.pwg"
cp "$out/label.pwg" "$out/truncated-second-header.pwg"
printf 'PwgR' >> "$out/truncated-second-header.pwg"
head -c 1810 "$out/label.pwg" > "$out/incomplete-page.pwg"
head -c -12 "$out/label.pdf" > "$out/truncated.pdf"
printf '%%PDF-1.4\ninvalid\n%%%%EOF\n' > "$out/invalid.pdf"
cp "$out/label.pdf" "$out/oversize.pdf"
truncate -s 8388609 "$out/oversize.pdf"
binary=${CARGO_TARGET_DIR:-"$root/target"}/debug/letratag-printer
"$binary" serve --state "$out/state" --spool "$out/spool" --capture --port "$port" > "$out/server.log" 2>&1 &
server=$!
cleanup() { kill "$server" 2>/dev/null || true;wait "$server" 2>/dev/null || true; }
trap cleanup EXIT
uri="ipp://127.0.0.1:$port/ipp/print"
ready=false
for _ in {1..50}; do
  if ipptool -T 2 "$uri" tests/ipp/get-printer.test > "$out/readiness.log" 2>&1; then ready=true;break;fi
  kill -0 "$server" 2>/dev/null || { cat "$out/server.log";exit 1; }
  sleep 0.1
done
$ready || { cat "$out/readiness.log";exit 1; }
ss -H -ltn "sport = :$port" > "$out/listeners.txt"
local_address=$(awk '{print $4}' "$out/listeners.txt")
[[ $local_address == "127.0.0.1:$port" ]]
ipptool -tv -T 5 "$uri" tests/ipp/get-printer.test > "$out/capabilities.log"
ipptool -tv -T 5 "ipp://127.0.0.1:$port/printers/letratag" tests/ipp/get-printer.test > "$out/named-uri.log"
ipptool -tv -T 5 "ipp://127.0.0.1:$port/printers/not-letratag" tests/ipp/unknown-printer.test > "$out/unknown-named-uri.log"
ipptool -tv -T 5 "$uri" tests/ipp/validate-reject-raster.test > "$out/validate-raster.log"
ipptool -tv -T 5 "$uri" tests/ipp/validate-options.test > "$out/validate-options.log"
# Rejections must not construct a transport or create an engine journal.
for fixture in truncated-header.pwg truncated-second-header.pwg incomplete-page.pwg label.pwg;do
  ipptool -tv -T 10 -f "$out/$fixture" "$uri" tests/ipp/reject-raster.test > "$out/reject-$fixture.log"
done
ipptool -tv -T 10 -f "$out/truncated-header.pwg" "$uri" tests/ipp/reject-urf.test > "$out/reject-urf.log"
ipptool -tv -T 10 -f "$out/label.pwg" "$uri" tests/ipp/reject-autodetected-raster.test > "$out/reject-auto.log"
ipptool -tv -T 10 -f "$out/oversize.pdf" "$uri" tests/ipp/reject-oversize.test > "$out/reject-oversize.log"
for fixture in truncated.pdf invalid.pdf two-pages.pdf;do
  ipptool -tv -T 20 -d length=5000 -f "$out/$fixture" "$uri" tests/ipp/print-and-expect-aborted.test > "$out/reject-$fixture.log"
done
ipptool -tv -T 20 -f "$out/label.pdf" "$uri" tests/ipp/reject-unsupported-media.test > "$out/reject-media-substitution.log"
ipptool -tv -T 20 -d length=4900 -f "$out/label.pdf" "$uri" tests/ipp/print-and-expect-aborted.test > "$out/reject-length.log"
"$out/ipp-client" "$port" "$out/label.pdf" short > "$out/reject-short-http.log"
"$out/ipp-client" "$port" "$out/label.pdf" chunked-short > "$out/reject-short-chunked.log"
sleep 0.5
[[ $(find "$out/state" -name '*.state' | wc -l) == 0 ]]
# Print submissions are never retried if an assertion fails.
ipptool -tv -T 20 -f "$out/label.pdf" "$uri" tests/ipp/print-pdf.test > "$out/pdf.log"
ipptool -tv -T 20 -f "$out/label.pdf" "$uri" tests/ipp/print-pdf.test > "$out/pdf-second.log"
lp -h "127.0.0.1:$port" -d letratag -o media=custom_default_12x50mm -o print-scaling=none "$out/label.pdf" > "$out/lp.log"
lpr -H "127.0.0.1:$port" -P letratag -o media=custom_default_12x50mm -o print-scaling=none "$out/label.pdf"
for _ in {1..100}; do
  count=$(find "$out/state" -name '*.state' -exec grep -l '^completed$' {} + 2>/dev/null | wc -l) || count=0
  if ((count==4));then break;fi
  sleep 0.1
done
[[ $count == 4 ]] || { cat "$out/server.log";echo "Expected 4 completed mock jobs, got $count" >&2;exit 1; }
[[ $(find "$out/state" -name '*.ble' | wc -l) == 4 ]]
[[ $(find "$out/state" -name '*.png' | wc -l) == 4 ]]
"$out/inspect-captures" "$out/state" > "$out/decoded-captures.log"
for i in {1..40};do
  ipptool -tv -T 10 -f "$out/invalid.pdf" "$uri" tests/ipp/flood-invalid.test > "$out/flood-$i.log"
done
ipptool -tv -T 5 "$uri" tests/ipp/all-jobs.test > "$out/all-jobs.log"
retained=$(grep -cE '^ *job-id \(integer\) =' "$out/all-jobs.log")
((retained>0&&retained<=32))
grep -q 'server-error-busy' "$out"/flood-*.log
[[ $(find "$out/state" -name '*.state' | wc -l) == 4 ]]
# Generic framework sink verifies byte preservation of a valid multipage PDF.
cc tests/pappl-policy.c crates/letratag-printer/native/adapter.c $(pkg-config --cflags --libs pappl) -o "$out/policy-server"
policy_port=$((port+1))
if ss -H -ltn "sport = :$policy_port" | grep -q .;then echo 'Policy port occupied' >&2;exit 1;fi
mkdir -m 700 "$out/policy-spool"
"$out/policy-server" "$out/policy-spool" "$policy_port" > "$out/policy.log" 2>&1 &
policy=$!
cleanup_all() { kill "$policy" 2>/dev/null || true;wait "$policy" 2>/dev/null || true;cleanup; }
trap cleanup_all EXIT
policy_uri="ipp://127.0.0.1:$policy_port/ipp/print"
for _ in {1..50};do
  if ipptool -T 2 "$policy_uri" tests/ipp/get-printer.test > /dev/null 2>&1;then break;fi
  sleep 0.1
done
ipptool -tv -T 20 -f "$out/two-pages.pdf" "$policy_uri" tests/ipp/print-pdf.test > "$out/multipage-preserved.log"
"$out/policy-server" "$out/two-pages.pdf" > "$out/expected-receipt.txt"
for _ in {1..50};do if grep -q '^RECEIVED ' "$out/policy.log";then break;fi;sleep 0.1;done
grep '^RECEIVED ' "$out/policy.log" > "$out/actual-receipt.txt"
cmp "$out/expected-receipt.txt" "$out/actual-receipt.txt"
printf 'PASS: PDF-only loopback IPP, invalid-input rejection, lp/lpr, 4 mock jobs, multipage byte preservation. Evidence: %s\n' "$out"
