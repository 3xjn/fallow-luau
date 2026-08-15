"use strict";

const fs = require("node:fs");
const https = require("node:https");
const path = require("node:path");
const { createWriteStream } = require("node:fs");
const { pipeline } = require("node:stream/promises");
const { execFileSync } = require("node:child_process");
const {
  resolveBinary,
  binaryName,
  releaseAssetName,
  rustTargetTriple,
  platformKey,
} = require("./platform.js");

const REPO = "3xjn/fallow-luau";
const pkg = require("../package.json");

function log(msg) {
  console.error(`fallow-luau: ${msg}`);
}

function get(url, redirects = 0) {
  return new Promise((resolve, reject) => {
    https
      .get(url, { headers: { "User-Agent": "fallow-luau-npm" } }, (res) => {
        if (
          res.statusCode >= 300 &&
          res.statusCode < 400 &&
          res.headers.location &&
          redirects < 5
        ) {
          res.resume();
          resolve(get(res.headers.location, redirects + 1));
          return;
        }
        if (res.statusCode !== 200) {
          reject(new Error(`HTTP ${res.statusCode} for ${url}`));
          res.resume();
          return;
        }
        resolve(res);
      })
      .on("error", reject);
  });
}

async function download(url, dest) {
  const res = await get(url);
  await pipeline(res, createWriteStream(dest));
}

function extractArchive(archivePath, destDir) {
  fs.mkdirSync(destDir, { recursive: true });
  if (archivePath.endsWith(".zip")) {
    if (process.platform === "win32") {
      execFileSync(
        "powershell.exe",
        [
          "-NoProfile",
          "-Command",
          `Expand-Archive -LiteralPath '${archivePath.replace(/'/g, "''")}' -DestinationPath '${destDir.replace(/'/g, "''")}' -Force`,
        ],
        { stdio: "ignore" },
      );
    } else {
      execFileSync("unzip", ["-o", archivePath, "-d", destDir], { stdio: "ignore" });
    }
  } else {
    execFileSync("tar", ["-xzf", archivePath, "-C", destDir], { stdio: "ignore" });
  }
}

function findExtractedBinary(dir) {
  const want = binaryName();
  const stack = [dir];
  while (stack.length) {
    const cur = stack.pop();
    for (const name of fs.readdirSync(cur)) {
      const full = path.join(cur, name);
      const st = fs.statSync(full);
      if (st.isDirectory()) stack.push(full);
      else if (name === want) return full;
    }
  }
  return null;
}

async function downloadFromGitHub() {
  if (!platformKey() || !rustTargetTriple()) {
    log(`unsupported platform ${process.platform}-${process.arch}`);
    return false;
  }

  const version = pkg.version;
  const asset = releaseAssetName(version);
  const tag = `v${version}`;
  const url = `https://github.com/${REPO}/releases/download/${tag}/${asset}`;
  const vendorDir = path.join(__dirname, "..", "vendor");
  const tmpDir = path.join(vendorDir, ".tmp");
  fs.mkdirSync(tmpDir, { recursive: true });
  const archivePath = path.join(tmpDir, asset);

  log(`optional platform package missing; downloading ${asset}`);
  try {
    await download(url, archivePath);
  } catch (err) {
    log(`could not download ${url}: ${err.message}`);
    log("publish a GitHub Release or install via cargo / install.sh");
    return false;
  }

  extractArchive(archivePath, tmpDir);
  const found = findExtractedBinary(tmpDir);
  if (!found) {
    log("downloaded archive did not contain fallow-luau binary");
    return false;
  }

  fs.mkdirSync(vendorDir, { recursive: true });
  const dest = path.join(vendorDir, binaryName());
  fs.copyFileSync(found, dest);
  try {
    fs.chmodSync(dest, 0o755);
  } catch {
    /* windows */
  }

  fs.rmSync(tmpDir, { recursive: true, force: true });
  return true;
}

async function main() {
  if (resolveBinary()) return;
  const ok = await downloadFromGitHub();
  if (!ok && !resolveBinary()) {
    // Soft-fail: leave a helpful message; `npx` still installs the package.
    log("binary not installed yet. After the first GitHub Release, reinstall or use:");
    log("  curl -fsSL https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.sh | bash");
    log("  irm https://raw.githubusercontent.com/3xjn/fallow-luau/main/scripts/install.ps1 | iex");
  }
}

main().catch((err) => {
  log(err.stack || String(err));
  // Do not fail npm install hard — optional binary fetch can retry later.
  process.exit(0);
});
