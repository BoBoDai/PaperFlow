#!/bin/bash
# PaperFlow - Start both backend and frontend
# Usage: ./start.sh

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}Starting PaperFlow...${NC}"

# Kill existing processes on port 8080
if lsof -ti:8080 > /dev/null 2>&1; then
    echo -e "${YELLOW}Stopping existing backend on port 8080...${NC}"
    lsof -ti:8080 | xargs kill 2>/dev/null || true
    sleep 1
fi

# Start Rust backend in background
echo -e "${GREEN}Starting Rust API server...${NC}"
cargo run --bin server &
BACKEND_PID=$!
echo "Backend PID: $BACKEND_PID"

# Wait for backend to start
sleep 3

# Check if backend is running
if ! lsof -ti:8080 > /dev/null 2>&1; then
    echo -e "${RED}Backend failed to start on port 8080${NC}"
    exit 1
fi

echo -e "${GREEN}Backend started on http://127.0.0.1:8080${NC}"

# Start Ink frontend
echo -e "${GREEN}Starting Ink frontend...${NC}"
cd ui && npm start &
FRONTEND_PID=$!
echo "Frontend PID: $FRONTEND_PID"

echo ""
echo -e "${GREEN}PaperFlow is running!${NC}"
echo -e "  Backend: http://127.0.0.1:8080"
echo -e "  Frontend: npm start"
echo ""
echo -e "Press ${RED}Ctrl+C${NC} to stop all services"

# Wait for any process to exit
wait $BACKEND_PID $FRONTEND_PID 2>/dev/null || true

# Cleanup on exit
echo -e "\n${YELLOW}Stopping PaperFlow...${NC}"
kill $BACKEND_PID 2>/dev/null || true
lsof -ti:8080 | xargs kill 2>/dev/null || true
echo -e "${GREEN}Done${NC}"
