#!/usr/bin/env node

const { spawn } = require("child_process");
const fs = require("fs");
const path = require("path");

const ext = process.platform === "win32" ? ".exe" : "";
const bin = path.join(__dirname, "bin", `prezzy${ext}`);

if (!fs.existsSync(bin)) {
  console.error("prezzy: binary not found. Try reinstalling: npm i -g prezzy-cli");
  process.exit(1);
}

const child = spawn(bin, process.argv.slice(2), { stdio: "inherit" });

child.on("exit", (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal);
  } else {
    process.exit(code ?? 1);
  }
});
