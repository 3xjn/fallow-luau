#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");
const { resolveBinary } = require("../scripts/platform.js");

const binary = resolveBinary();
if (!binary) {
  console.error(
    [
      "fallow-luau: no platform binary found.",
      "Install a prebuilt package, or use one of:",
      "  curl -fsSL https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.sh | bash",
      "  irm https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.ps1 | iex",
      "  cargo install --git https://github.com/3xjn/fallow-luau",
    ].join("\n"),
  );
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: true,
});

if (result.error) {
  console.error(`fallow-luau: failed to launch ${binary}: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status === null ? 1 : result.status);
