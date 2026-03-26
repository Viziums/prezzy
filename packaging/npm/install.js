#!/usr/bin/env node

// Downloads the correct prezzy binary for the current platform.

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");
const https = require("https");

const VERSION = require("./package.json").version;
const REPO = "viziums/prezzy";

const TARGETS = {
  "darwin-x64": "x86_64-apple-darwin",
  "darwin-arm64": "aarch64-apple-darwin",
  "linux-x64": "x86_64-unknown-linux-gnu",
  "linux-arm64": "aarch64-unknown-linux-gnu",
  "win32-x64": "x86_64-pc-windows-msvc",
};

const platform = `${process.platform}-${process.arch}`;
const target = TARGETS[platform];

if (!target) {
  console.error(`prezzy: unsupported platform ${platform}`);
  console.error("Install from source: cargo install prezzy");
  process.exit(1);
}

const ext = process.platform === "win32" ? "zip" : "tar.gz";
const url = `https://github.com/${REPO}/releases/download/v${VERSION}/prezzy-${target}.${ext}`;
const binDir = path.join(__dirname, "bin");
const binPath = path.join(binDir, process.platform === "win32" ? "prezzy.exe" : "prezzy");

if (fs.existsSync(binPath)) {
  process.exit(0); // Already installed.
}

fs.mkdirSync(binDir, { recursive: true });

console.log(`Downloading prezzy v${VERSION} for ${platform}...`);

if (ext === "tar.gz") {
  execSync(`curl -sL "${url}" | tar xz -C "${binDir}"`, { stdio: "inherit" });
} else {
  const zipPath = path.join(binDir, "prezzy.zip");
  execSync(`curl -sL "${url}" -o "${zipPath}"`, { stdio: "inherit" });
  execSync(`tar -xf "${zipPath}" -C "${binDir}"`, { stdio: "inherit" });
  fs.unlinkSync(zipPath);
}

fs.chmodSync(binPath, 0o755);
console.log("prezzy installed successfully.");
