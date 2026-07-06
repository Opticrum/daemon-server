#!/usr/bin/env bash
# Prepare sibling-repo paths expected by Cargo.toml path dependencies.
#
# Layout (relative to $ROOT):
#   Freelancer/fiber/rust-server   — this repo (daemon-server)
#   Freelancer/fiber/opticrum      — Opticrum/ckb-contract-script
#   Freelancer/ckb-cinnabar        — ashuralyk/ckb-cinnabar
#   Cryptape/fiber/.../fiber-json-types — vendored from nervosnetwork/fiber

set -euo pipefail

ROOT="${1:?usage: ci-checkout-deps.sh <workspace-root>}"
FIBER_JSON_TYPES_REF="${FIBER_JSON_TYPES_REF:-master}"

mkdir -p "${ROOT}/Freelancer/fiber" "${ROOT}/Freelancer" "${ROOT}/Cryptape/fiber/lightning-network/fiber/crates"

if [[ ! -d "${ROOT}/Freelancer/fiber/opticrum/.git" ]]; then
  git clone --depth 1 https://github.com/Opticrum/ckb-contract-script.git \
    "${ROOT}/Freelancer/fiber/opticrum"
fi

if [[ ! -d "${ROOT}/Freelancer/ckb-cinnabar/.git" ]]; then
  git clone --depth 1 https://github.com/ashuralyk/ckb-cinnabar.git \
    "${ROOT}/Freelancer/ckb-cinnabar"
fi

FIBER_JSON_TYPES_DIR="${ROOT}/Cryptape/fiber/lightning-network/fiber/crates/fiber-json-types"
if [[ ! -f "${FIBER_JSON_TYPES_DIR}/Cargo.toml" ]]; then
  tmp_fiber="$(mktemp -d)"
  git clone --depth 1 --branch "${FIBER_JSON_TYPES_REF}" \
    https://github.com/nervosnetwork/fiber.git "${tmp_fiber}"
  rm -rf "${FIBER_JSON_TYPES_DIR}"
  cp -R "${tmp_fiber}/crates/fiber-json-types" "${FIBER_JSON_TYPES_DIR}"
  rm -rf "${tmp_fiber}"
fi

echo "Dependency tree ready under ${ROOT}"
