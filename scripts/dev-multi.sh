#!/bin/bash
# Multi-instance dev mode: runs two feiq++ instances on different ports
# Usage: ./scripts/dev-multi.sh

set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

echo "=== Building frontend ==="
npm --prefix packages/feiq-gui run build 2>/dev/null

echo "=== Building Rust binary ==="
cargo build --workspace 2>/dev/null

echo ""
echo "=== Starting Alice (port 2425) ==="
FEIQ_NAME=Alice ./target/debug/feiq-app &
ALICE_PID=$!

sleep 2

echo "=== Starting Bob (port 2426) ==="
FEIQ_NAME=Bob FEIQ_PORT=2426 ./target/debug/feiq-app &
BOB_PID=$!

echo ""
echo "Both instances running:"
echo "  Alice: PID $ALICE_PID, port 2425"
echo "  Bob:   PID $BOB_PID,   port 2426"
echo ""
echo "Bob should auto-discover on 2425 broadcast."
echo "If not, manually add: Alice + '127.0.0.1:2426', Bob + '127.0.0.1:2425'"
echo ""
echo "Press Enter to stop both..."

read

kill $ALICE_PID $BOB_PID 2>/dev/null
echo "Stopped."
