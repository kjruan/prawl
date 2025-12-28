#!/bin/bash
# Find wireless interfaces in monitor mode

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}Scanning for wireless interfaces...${NC}"
echo ""

FOUND_MONITOR=false

# Method 1: Use iw to list all wireless devices
for iface in $(iw dev 2>/dev/null | grep "Interface" | awk '{print $2}'); do
    # Get interface info
    info=$(iw dev "$iface" info 2>/dev/null)
    type=$(echo "$info" | grep "type" | awk '{print $2}')
    channel=$(echo "$info" | grep "channel" | awk '{print $2}')

    if [ "$type" = "monitor" ]; then
        echo -e "${GREEN}[MONITOR]${NC} $iface (channel: ${channel:-N/A})"
        FOUND_MONITOR=true
    else
        echo -e "${YELLOW}[${type:-unknown}]${NC} $iface"
    fi
done

# Method 2: Check /sys/class/net for wireless interfaces (fallback)
if [ -z "$(iw dev 2>/dev/null)" ]; then
    for iface in /sys/class/net/*; do
        iface=$(basename "$iface")
        # Check if it's a wireless interface
        if [ -d "/sys/class/net/$iface/wireless" ] || [ -L "/sys/class/net/$iface/phy80211" ]; then
            # Try to get mode from iwconfig
            mode=$(iwconfig "$iface" 2>/dev/null | grep "Mode:" | sed 's/.*Mode:\([^ ]*\).*/\1/')
            if [ "$mode" = "Monitor" ]; then
                echo -e "${GREEN}[MONITOR]${NC} $iface"
                FOUND_MONITOR=true
            else
                echo -e "${YELLOW}[${mode:-unknown}]${NC} $iface"
            fi
        fi
    done
fi

echo ""

if [ "$FOUND_MONITOR" = true ]; then
    echo -e "${GREEN}Monitor mode interface(s) found!${NC}"
    # Output just the interface name for scripting
    if [ "$1" = "-q" ] || [ "$1" = "--quiet" ]; then
        iw dev 2>/dev/null | grep -A 2 "Interface" | grep -B 1 "type monitor" | grep "Interface" | awk '{print $2}' | head -1
    fi
else
    echo -e "${YELLOW}No monitor mode interfaces found.${NC}"
    echo ""
    echo "To enable monitor mode:"
    echo "  sudo ip link set <interface> down"
    echo "  sudo iw dev <interface> set type monitor"
    echo "  sudo ip link set <interface> up"
    echo ""
    echo "Or use prowl:"
    echo "  sudo prowl tui --set-monitor"
    exit 1
fi
