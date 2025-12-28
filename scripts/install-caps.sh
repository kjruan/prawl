#!/bin/bash
# Install prowl with capabilities for non-root packet capture

set -e

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

BINARY="${1:-./target/release/prowl}"
INSTALL_PATH="${2:-/usr/local/bin/prowl}"

if [ ! -f "$BINARY" ]; then
    echo -e "${RED}Error: Binary not found at $BINARY${NC}"
    echo "Build first with: ./build.sh -r"
    exit 1
fi

echo -e "${YELLOW}Installing prowl with network capabilities...${NC}"

# Copy binary
sudo cp "$BINARY" "$INSTALL_PATH"
sudo chmod 755 "$INSTALL_PATH"

# Set capabilities for packet capture and network admin (monitor mode)
sudo setcap 'cap_net_raw,cap_net_admin=eip' "$INSTALL_PATH"

# Verify
echo ""
echo -e "${GREEN}Installed to: $INSTALL_PATH${NC}"
echo ""
echo "Capabilities set:"
getcap "$INSTALL_PATH"

echo ""
echo -e "${GREEN}Done! You can now run prowl without sudo:${NC}"
echo "  prowl scan"
echo "  prowl capture"
echo "  prowl tui"
echo ""
echo -e "${YELLOW}Note: Setting monitor mode still requires sudo:${NC}"
echo "  sudo prowl capture --set-monitor  # First time only"
echo "  prowl capture                     # After interface is in monitor mode"
