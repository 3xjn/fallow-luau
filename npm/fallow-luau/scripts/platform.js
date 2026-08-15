"use strict";

const fs = require("node:fs");
const path = require("node:path");

const PACKAGE_BY_TARGET = {
  "darwin-arm64": "@fallow-luau/darwin-arm64",
  "darwin-x64": "@fallow-luau/darwin-x64",
  "linux-arm64": "@fallow-luau/linux-arm64",
  "linux-x64": "@fallow-luau/linux-x64",
  "win32-x64": "@fallow-luau/win32-x64",
};

function platformKey() {
  const { platform, arch } = process;
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && (arch === "x64" || arch === "ia32")) return "darwin-x64";
  if (platform === "linux" && arch === "arm64") return "linux-arm64";
  if (platform === "linux" && arch === "x64") return "linux-x64";
  if (platform === "win32" && (arch === "x64" || arch === "ia32")) return "win32-x64";
  return null;
}

function binaryName() {
  return process.platform === "win32" ? "fallow-luau.exe" : "fallow-luau";
}

function tryRequireResolve(pkg) {
  try {
    return require.resolve(`${pkg}/package.json`);
  } catch {
    return null;
  }
}

function resolveFromOptionalDep() {
  const key = platformKey();
  if (!key) return null;
  const pkg = PACKAGE_BY_TARGET[key];
  const pkgJson = tryRequireResolve(pkg);
  if (!pkgJson) return null;
  const candidate = path.join(path.dirname(pkgJson), "bin", binaryName());
  return fs.existsSync(candidate) ? candidate : null;
}

function resolveFromVendor() {
  const candidate = path.join(__dirname, "..", "vendor", binaryName());
  return fs.existsSync(candidate) ? candidate : null;
}

function resolveBinary() {
  return resolveFromOptionalDep() || resolveFromVendor();
}

function rustTargetTriple() {
  const key = platformKey();
  switch (key) {
    case "darwin-arm64":
      return "aarch64-apple-darwin";
    case "darwin-x64":
      return "x86_64-apple-darwin";
    case "linux-arm64":
      return "aarch64-unknown-linux-gnu";
    case "linux-x64":
      return "x86_64-unknown-linux-gnu";
    case "win32-x64":
      return "x86_64-pc-windows-msvc";
    default:
      return null;
  }
}

function releaseAssetName(version) {
  const triple = rustTargetTriple();
  if (!triple) return null;
  const ext = process.platform === "win32" ? "zip" : "tar.gz";
  return `fallow-luau-${version}-${triple}.${ext}`;
}

module.exports = {
  PACKAGE_BY_TARGET,
  platformKey,
  binaryName,
  resolveBinary,
  rustTargetTriple,
  releaseAssetName,
};
