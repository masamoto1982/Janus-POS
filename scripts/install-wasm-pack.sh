#!/usr/bin/env bash
set -euo pipefail

version="${WASM_PACK_VERSION:-0.13.1}"
install_dir="${CARGO_HOME:-${HOME}/.cargo}/bin"
binary="${install_dir}/wasm-pack"

if command -v wasm-pack >/dev/null 2>&1; then
  installed_version="$(wasm-pack --version | awk '{print $2}')"
  if [[ "${installed_version}" == "${version}" ]]; then
    echo "install-wasm-pack: wasm-pack ${version} is already installed at $(command -v wasm-pack)."
    exit 0
  fi
  echo "install-wasm-pack: found wasm-pack ${installed_version}; installing expected ${version}."
fi

release_target=""
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)
    release_target="x86_64-unknown-linux-musl"
    ;;
esac

install_from_cargo() {
  if ! command -v cargo >/dev/null 2>&1; then
    echo "install-wasm-pack: cargo is unavailable and release archive install failed." >&2
    return 1
  fi

  echo "install-wasm-pack: falling back to cargo install wasm-pack ${version}."
  cargo install wasm-pack --locked --version "${version}"
}

if [[ -n "${release_target}" ]]; then
  tmp_dir="$(mktemp -d)"
  trap 'rm -rf "${tmp_dir}"' EXIT

  archive="${tmp_dir}/wasm-pack.tar.gz"
  package_dir="${tmp_dir}/wasm-pack-v${version}-${release_target}"
  release_url="https://github.com/rustwasm/wasm-pack/releases/download/v${version}/wasm-pack-v${version}-${release_target}.tar.gz"

  echo "install-wasm-pack: downloading ${release_url}"
  if curl --fail --location --retry 5 --retry-delay 5 --retry-all-errors \
    --connect-timeout 20 --max-time 300 --output "${archive}" "${release_url}"; then
    tar -xzf "${archive}" -C "${tmp_dir}"
    mkdir -p "${install_dir}"
    cp "${package_dir}/wasm-pack" "${binary}"
    chmod +x "${binary}"
    echo "install-wasm-pack: installed wasm-pack ${version} to ${binary}."
    exit 0
  fi

  echo "install-wasm-pack: release download failed after retries." >&2
else
  echo "install-wasm-pack: no prebuilt release mapping for $(uname -s)-$(uname -m)." >&2
fi

install_from_cargo
