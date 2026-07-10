#!/bin/bash
# Start Coleoptera Desktop App

BASE_DIR="$(cd "$(dirname "$0")" && pwd)"

echo "Starting Python backend..."
"$BASE_DIR/.venv/bin/python" "$BASE_DIR/backend.py" &
BACKEND_PID=$!

echo "Starting Electron frontend..."
cd "$BASE_DIR/electron"
npm run dev

echo "Stopping Python backend (PID: $BACKEND_PID)..."
kill $BACKEND_PID 2>/dev/null
