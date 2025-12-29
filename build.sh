#!/bin/bash
set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

echo -e "${YELLOW}Building prowl...${NC}"

# Check if cargo is available
if ! command -v cargo &> /dev/null; then
    echo -e "${RED}Error: cargo not found. Please install Rust.${NC}"
    exit 1
fi

# Helper to install cargo-zigbuild if needed
install_zigbuild() {
    if ! command -v cargo-zigbuild &>/dev/null; then
        echo -e "${YELLOW}Installing cargo-zigbuild for cross-compilation...${NC}"
        cargo install cargo-zigbuild
    fi

    if ! command -v zig &>/dev/null; then
        echo ""
        echo -e "${RED}ERROR: zig is required for cross-compilation${NC}"
        echo "Install with your package manager:"
        echo "  Arch:   sudo pacman -S zig"
        echo "  Ubuntu: sudo snap install zig --classic"
        echo "  macOS:  brew install zig"
        exit 1
    fi
}

# Parse arguments
RELEASE=false
INSTALL=false
CLEAN=false
CAPS=false
TARGET=""
RPI_32=false
RPI_64=false
USE_ZIGBUILD=false

for arg in "$@"; do
    case $arg in
        --release|-r)
            RELEASE=true
            ;;
        --install|-i)
            INSTALL=true
            RELEASE=true  # Install implies release
            ;;
        --clean|-c)
            CLEAN=true
            ;;
        --rpi|--rpi32|--pi)
            RPI_32=true
            RELEASE=true
            USE_ZIGBUILD=true
            TARGET="armv7-unknown-linux-gnueabihf"
            ;;
        --rpi64|--pi64)
            RPI_64=true
            RELEASE=true
            USE_ZIGBUILD=true
            TARGET="aarch64-unknown-linux-gnu"
            ;;
        --caps)
            CAPS=true
            INSTALL=true
            RELEASE=true
            ;;
        --target)
            # Next arg will be the target
            ;;
        --help|-h)
            echo "Usage: ./build.sh [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  -r, --release      Build in release mode (optimized)"
            echo "  -i, --install      Build and install to /usr/local/bin (requires sudo)"
            echo "  -c, --clean        Clean build artifacts before building"
            echo "  --rpi, --rpi32     Cross-compile for Raspberry Pi (32-bit ARM)"
            echo "  --rpi64            Cross-compile for Raspberry Pi (64-bit ARM)"
            echo "  --caps             Install with capabilities (run without sudo)"
            echo "  -h, --help         Show this help message"
            echo ""
            echo "Cross-compilation requirements:"
            echo "  sudo pacman -S zig    # Arch"
            echo "  cargo install cargo-zigbuild"
            exit 0
            ;;
        *)
            echo -e "${RED}Unknown option: $arg${NC}"
            exit 1
            ;;
    esac
done

# Setup cross-compilation if targeting Raspberry Pi
if [ -n "$TARGET" ]; then
    echo -e "${CYAN}Target: ${TARGET}${NC}"
    install_zigbuild

    # Check if target is installed
    if ! rustup target list --installed | grep -q "$TARGET"; then
        echo -e "${YELLOW}Installing Rust target: ${TARGET}${NC}"
        rustup target add "$TARGET"
    fi
fi

# Clean if requested
if [ "$CLEAN" = true ]; then
    echo -e "${YELLOW}Cleaning build artifacts...${NC}"
    cargo clean
fi

# Build
if [ -n "$TARGET" ]; then
    echo -e "${YELLOW}Building release binary for ${TARGET} (using zigbuild)...${NC}"
    cargo zigbuild --release --target "$TARGET"
    BINARY="target/${TARGET}/release/prowl"
elif [ "$RELEASE" = true ]; then
    echo -e "${YELLOW}Building release binary...${NC}"
    cargo build --release
    BINARY="target/release/prowl"
else
    echo -e "${YELLOW}Building debug binary...${NC}"
    cargo build
    BINARY="target/debug/prowl"
fi

# Check build success
if [ -f "$BINARY" ]; then
    echo -e "${GREEN}Build successful!${NC}"
    echo -e "Binary: ${BINARY}"

    # Show binary size
    SIZE=$(du -h "$BINARY" | cut -f1)
    echo -e "Size: ${SIZE}"
else
    echo -e "${RED}Build failed!${NC}"
    exit 1
fi

# Install if requested (only for native builds)
if [ "$INSTALL" = true ]; then
    if [ -n "$TARGET" ]; then
        echo -e "${YELLOW}Skipping install for cross-compiled binary.${NC}"
        echo -e "Copy to your Raspberry Pi with:"
        echo -e "  scp ${BINARY} pi@<raspberry-pi-ip>:/usr/local/bin/prowl"
    else
        echo -e "${YELLOW}Installing to /usr/local/bin/prowl...${NC}"
        sudo cp "$BINARY" /usr/local/bin/prowl
        sudo chmod +x /usr/local/bin/prowl

        # Set capabilities if requested
        if [ "$CAPS" = true ]; then
            echo -e "${YELLOW}Setting network capabilities...${NC}"
            sudo setcap 'cap_net_raw,cap_net_admin=eip' /usr/local/bin/prowl
            echo -e "${GREEN}Capabilities set:${NC}"
            getcap /usr/local/bin/prowl
            echo ""
            echo -e "${GREEN}You can now run prowl without sudo!${NC}"
            echo -e "${YELLOW}Note: First-time monitor mode still needs sudo:${NC}"
            echo -e "  sudo ip link set wlan0 down"
            echo -e "  sudo iw dev wlan0 set type monitor"
            echo -e "  sudo ip link set wlan0 up"
            echo -e "Then run normally: prowl tui"
        else
            echo -e "${GREEN}Installed successfully!${NC}"
            echo -e "Run 'prowl --help' to get started."
        fi
    fi
fi

# Show copy instructions for cross-compiled binaries
if [ -n "$TARGET" ]; then
    echo ""
    echo -e "${CYAN}To deploy to Raspberry Pi:${NC}"
    echo -e "  scp ${BINARY} pi@<raspberry-pi-ip>:~/"
    echo -e "  ssh pi@<raspberry-pi-ip> 'sudo mv ~/prowl /usr/local/bin/ && sudo chmod +x /usr/local/bin/prowl'"
fi

echo ""
echo -e "${GREEN}Done!${NC}"
