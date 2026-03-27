#!/usr/bin/env node

// Downloads the correct prezzy binary for the current platform.

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const VERSION = require("./package.json").version;
const REPO = "viziums/prezzy";

function isMusl() {
  if (process.platform !== "linux") return false;
  try {
    const lddOutput = execSync("ldd --version 2>&1", { encoding: "utf8" });
    return lddOutput.toLowerCase().includes("musl");
  } catch {
    // ldd --version exits non-zero on musl systems
    try {
      const stderr = execSync("ldd --version 2>&1 || true", {
        encoding: "utf8",
      });
      return stderr.toLowerCase().includes("musl");
    } catch {
      // If ldd isn't available, check for Alpine's /etc/os-release
      try {
        const release = fs.readFileSync("/etc/os-release", "utf8");
        return release.toLowerCase().includes("alpine");
      } catch {
        return false;
      }
    }
  }
}

function getTarget() {
  const arch = process.arch;
  const platform = process.platform;

  if (platform === "darwin") {
    return arch === "arm64"
      ? "aarch64-apple-darwin"
      : "x86_64-apple-darwin";
  }

  if (platform === "linux") {
    const libc = isMusl() ? "musl" : "gnu";
    return arch === "arm64"
      ? `aarch64-unknown-linux-${libc}`
      : `x86_64-unknown-linux-${libc}`;
  }

  if (platform === "win32" && arch === "x64") {
    return "x86_64-pc-windows-msvc";
  }

  return null;
}

const target = getTarget();

if (!target) {
  console.error(
    `prezzy: unsupported platform ${process.platform}-${process.arch}`
  );
  console.error("Install from source: cargo install prezzy");
  process.exit(1);
}

const ext = process.platform === "win32" ? "zip" : "tar.gz";
const url = `https://github.com/${REPO}/releases/download/v${VERSION}/prezzy-${target}.${ext}`;
const binDir = path.join(__dirname, "bin");
const binName = process.platform === "win32" ? "prezzy.exe" : "prezzy";
const binPath = path.join(binDir, binName);

if (fs.existsSync(binPath)) {
  process.exit(0); // Already installed.
}

fs.mkdirSync(binDir, { recursive: true });

console.log(`Downloading prezzy v${VERSION} for ${target}...`);

try {
  if (ext === "tar.gz") {
    execSync(`curl -fsSL "${url}" | tar xz -C "${binDir}"`, {
      stdio: "inherit",
    });
  } else {
    const zipPath = path.join(binDir, "prezzy.zip");
    execSync(`curl -fsSL "${url}" -o "${zipPath}"`, { stdio: "inherit" });
    execSync(
      `powershell -NoProfile -Command "Expand-Archive -Force '${zipPath}' '${binDir}'"`,
      { stdio: "inherit" }
    );
    fs.unlinkSync(zipPath);
  }
} catch (err) {
  console.error(`\nFailed to download prezzy v${VERSION} for ${target}.`);
  console.error(`URL: ${url}`);
  console.error(
    "Install from source instead: cargo install prezzy\n"
  );
  process.exit(1);
}

if (!fs.existsSync(binPath)) {
  console.error(
    `Expected binary not found at ${binPath} after extraction.`
  );
  console.error(
    "The release archive may have an unexpected structure."
  );
  process.exit(1);
}

if (process.platform === "win32") {
  // npm resolves the "bin" entry to bin/prezzy (no .exe), so create a shell
  // shim that forwards to the real .exe. This lets npm's cmd wrapper find it.
  const shimPath = path.join(binDir, "prezzy");
  if (!fs.existsSync(shimPath)) {
    fs.writeFileSync(
      shimPath,
      '#!/bin/sh\nexec "$(dirname "$0")/prezzy.exe" "$@"\n'
    );
  }
} else {
  fs.chmodSync(binPath, 0o755);
}
console.log("prezzy installed successfully.");
