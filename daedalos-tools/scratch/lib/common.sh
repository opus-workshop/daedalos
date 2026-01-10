# Common utilities for scratch tool

SCRATCH_VERSION="1.0.0"
SCRATCH_ROOT="${SCRATCH_ROOT:-$HOME/.local/share/daedalos/scratch}"
SCRATCH_META="$SCRATCH_ROOT/meta.json"
DEFAULT_EXPIRY_HOURS=24

# Ensure directories exist
mkdir -p "$SCRATCH_ROOT"

# Colors
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    DIM='\033[2m'
    NC='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' CYAN='' BOLD='' DIM='' NC=''
fi

log_info() { echo -e "${BLUE}info:${NC} $*"; }
log_success() { echo -e "${GREEN}success:${NC} $*"; }
log_error() { echo -e "${RED}error:${NC} $*" >&2; }
log_warn() { echo -e "${YELLOW}warning:${NC} $*"; }
die() { log_error "$*"; exit 1; }

# Initialize meta file if not exists
init_meta() {
    if [[ ! -f "$SCRATCH_META" ]]; then
        echo '{"scratches":{}}' > "$SCRATCH_META"
    fi
}

# Get scratch path
get_scratch_path() {
    echo "$SCRATCH_ROOT/$1"
}

# Check if scratch exists
scratch_exists() {
    local name="$1"
    [[ -d "$(get_scratch_path "$name")" ]]
}

# Get scratch original path
get_scratch_original() {
    local name="$1"
    init_meta
    python3 -c "import json; d=json.load(open('$SCRATCH_META')); print(d.get('scratches',{}).get('$name',{}).get('original',''))"
}

# Get scratch mode
get_scratch_mode() {
    local name="$1"
    init_meta
    python3 -c "import json; d=json.load(open('$SCRATCH_META')); print(d.get('scratches',{}).get('$name',{}).get('mode','unknown'))"
}

# Record scratch metadata
record_scratch() {
    local name="$1"
    local original="$2"
    local mode="$3"

    init_meta

    local created=$(date -Iseconds)
    local expires=$(date -v+${DEFAULT_EXPIRY_HOURS}H -Iseconds 2>/dev/null || date -d "+${DEFAULT_EXPIRY_HOURS} hours" -Iseconds 2>/dev/null || echo "")

    python3 << EOF
import json
with open('$SCRATCH_META', 'r+') as f:
    data = json.load(f)
    if 'scratches' not in data:
        data['scratches'] = {}
    data['scratches']['$name'] = {
        'original': '$original',
        'mode': '$mode',
        'created': '$created',
        'expires': '$expires'
    }
    f.seek(0)
    json.dump(data, f, indent=2)
    f.truncate()
EOF
}

# Remove scratch record
remove_scratch_record() {
    local name="$1"
    init_meta

    python3 << EOF
import json
with open('$SCRATCH_META', 'r+') as f:
    data = json.load(f)
    if 'scratches' in data and '$name' in data['scratches']:
        del data['scratches']['$name']
    f.seek(0)
    json.dump(data, f, indent=2)
    f.truncate()
EOF
}

# List all scratches
list_scratches() {
    init_meta
    python3 -c "import json; d=json.load(open('$SCRATCH_META')); print('\n'.join(d.get('scratches',{}).keys()))"
}

# Detect best mode for source
detect_mode() {
    local source="$1"

    # Check if on Btrfs
    if command -v btrfs &>/dev/null; then
        if btrfs subvolume show "$source" &>/dev/null 2>&1; then
            echo "btrfs"
            return
        fi
    fi

    # Check if git repo
    if git -C "$source" rev-parse --is-inside-work-tree &>/dev/null 2>&1; then
        echo "git"
        return
    fi

    # Fallback to copy
    echo "copy"
}

# Time utilities
time_ago() {
    local timestamp="$1"
    local now=$(date +%s)
    local created=$(date -j -f "%Y-%m-%dT%H:%M:%S" "${timestamp%[+-]*}" +%s 2>/dev/null || echo "$now")
    local diff=$((now - created))

    if [[ $diff -lt 60 ]]; then
        echo "just now"
    elif [[ $diff -lt 3600 ]]; then
        echo "$((diff/60)) min ago"
    elif [[ $diff -lt 86400 ]]; then
        echo "$((diff/3600)) hours ago"
    else
        echo "$((diff/86400)) days ago"
    fi
}

is_expired() {
    local name="$1"
    init_meta

    local expires=$(python3 -c "import json; d=json.load(open('$SCRATCH_META')); print(d.get('scratches',{}).get('$name',{}).get('expires',''))")

    if [[ -z "$expires" ]]; then
        return 1  # No expiry = not expired
    fi

    local expires_epoch=$(date -j -f "%Y-%m-%dT%H:%M:%S" "${expires%[+-]*}" +%s 2>/dev/null || echo "0")
    local now=$(date +%s)

    [[ $now -gt $expires_epoch ]]
}
